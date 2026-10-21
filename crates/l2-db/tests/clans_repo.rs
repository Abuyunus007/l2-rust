#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `ClanRepository` against a real MySQL/MariaDB server: Java's clan
//! statements (`Clan.restore*`, `store`, `updateClanInDB`, `removeMemberInDatabase`,
//! `setRankPrivs`, `storeNotice`, `ClanTable.destroyClan`, ...) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{ClanRepository, ClanUpdate, CrestRepository, NewClan, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 8] = [
    include_str!("../sql/game/characters.sql"),
    include_str!("../sql/game/clan_data.sql"),
    include_str!("../sql/game/clan_privs.sql"),
    include_str!("../sql/game/clan_subpledges.sql"),
    include_str!("../sql/game/clan_skills.sql"),
    include_str!("../sql/game/clan_notices.sql"),
    include_str!("../sql/game/clan_wars.sql"),
    include_str!("../sql/game/crests.sql"),
];

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
            "l2_test_clans_{}_{}_{}",
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

    async fn exec(&self, sql: &str) {
        sqlx::query(AssertSqlSafe(sql.to_owned()))
            .execute(&self.pool)
            .await
            .unwrap();
    }

    /// One number of a `SELECT`, as a signed `BIGINT` (the columns are unsigned).
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
async fn clan_rows_round_trip_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = ClanRepository::new(db.pool.clone());

    // Clan.store
    repo.insert_clan(&NewClan {
        clan_id: 900,
        clan_name: "Lions".into(),
        clan_level: 0,
        castle_id: 0,
        blood_alliance_count: 0,
        blood_oath_count: 0,
        ally_id: 0,
        ally_name: None,
        leader_id: 1001,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
        new_leader_id: 0,
    })
    .await
    .unwrap();
    assert_eq!(repo.clan_ids().await.unwrap(), [900]);
    assert_eq!(repo.leader_id(900).await.unwrap(), Some(1001));

    // Members are the characters with the clan id.
    db.exec(
        "INSERT INTO characters (account_name, charId, char_name, level, classid, clanid, title, power_grade, \
         subpledge, apprentice, sponsor, sex, race) VALUES \
         ('a', 1001, 'Leader', 40, 1, 900, NULL, 1, 0, 0, 0, 0, 1), \
         ('b', 1002, 'Cub', 20, 0, 900, 'cub', 9, -1, 0, 1003, 1, 2), \
         ('c', 1003, 'Sponsor', 50, 2, 900, NULL, 5, 0, 1002, 0, 0, 0)",
    )
    .await;
    let members = repo.clan_members(900).await.unwrap();
    assert_eq!(members.len(), 3);
    let cub = members.iter().find(|m| m.char_id == 1002).unwrap();
    assert_eq!(
        (
            cub.char_name.as_str(),
            cub.subpledge,
            cub.power_grade,
            cub.sponsor,
            cub.sex
        ),
        ("Cub", -1, Some(9), 1003, Some(1))
    );

    // Clan.updateClanInDB and changeLevel
    repo.update_clan(&ClanUpdate {
        clan_id: 900,
        leader_id: 1003,
        ally_id: 0,
        ally_name: None,
        reputation_score: -50,
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        char_penalty_expiry_time: 1_700_000_000_000,
        dissolving_expiry_time: 0,
        new_leader_id: 1002,
    })
    .await
    .unwrap();
    repo.update_clan_level(900, 5).await.unwrap();
    let data = repo.clan_data(900).await.unwrap().unwrap();
    assert_eq!(
        (
            data.leader_id,
            data.reputation_score,
            data.clan_level,
            data.new_leader_id
        ),
        (Some(1003), -50, Some(5), 1002)
    );
    assert_eq!(data.char_penalty_expiry_time, 1_700_000_000_000);

    // Sub pledges, ranks, notice
    repo.insert_sub_pledge(900, -1, "Cubs", 0).await.unwrap();
    repo.update_sub_pledge(900, -1, "Kittens", 1003).await.unwrap();
    let subs = repo.sub_pledges(900).await.unwrap();
    assert_eq!(
        (subs[0].sub_pledge_id, subs[0].name.as_deref(), subs[0].leader_id),
        (-1, Some("Kittens"), 1003)
    );
    repo.replace_rank_privs(900, 5, 0x46).await.unwrap();
    repo.replace_rank_privs(900, 5, 0x40).await.unwrap();
    let privs = repo.rank_privs(900).await.unwrap();
    assert_eq!(
        (privs.len(), privs[0].rank, privs[0].privs, privs[0].party),
        (1, 5, 0x40, 0)
    );
    repo.store_notice(900, "Hello", true).await.unwrap();
    repo.store_notice(900, "Bye", false).await.unwrap();
    let notices = repo.clan_notices(900).await.unwrap();
    assert_eq!(
        (notices[0].enabled.as_str(), notices[0].notice.as_str()),
        ("false", "Bye")
    );

    // The clan columns of the characters.
    repo.update_char_power_grade(1003, 3).await.unwrap();
    repo.update_char_subpledge(1003, 100).await.unwrap();
    repo.update_char_clan_privs(1003, 0x00FF_FFFF).await.unwrap();
    repo.update_char_apprentice_sponsor(1001, 5, 6).await.unwrap();
    assert_eq!(
        db.scalar("SELECT power_grade * 1000 + subpledge FROM characters WHERE charId = 1003")
            .await,
        3100
    );
    assert_eq!(
        db.scalar("SELECT clan_privs FROM characters WHERE charId = 1003")
            .await,
        0x00FF_FFFF
    );

    // Clan.removeMemberInDatabase: the member and the links to it.
    repo.remove_member(1002, 111, 222).await.unwrap();
    assert_eq!(
        db.scalar(
            "SELECT clanid + subpledge + clan_join_expiry_time + clan_create_expiry_time \
             FROM characters WHERE charId = 1002"
        )
        .await,
        333
    );
    assert_eq!(
        db.scalar("SELECT apprentice FROM characters WHERE charId = 1003")
            .await,
        0
    );

    // ClanTable.restoreClanWars reads the VARCHAR ids.
    db.exec("INSERT INTO clan_wars (clan1, clan2) VALUES ('900', '901')")
        .await;
    let wars = repo.clan_wars().await.unwrap();
    assert_eq!((wars[0].clan1.as_str(), wars[0].clan2.as_str()), ("900", "901"));
    db.exec("INSERT INTO clan_skills (clan_id, skill_id, skill_level, skill_name, sub_pledge_id) VALUES (900, 370, 1, 'x', -2)")
        .await;
    let skills = repo.clan_skills(900).await.unwrap();
    assert_eq!((skills[0].skill_id, skills[0].sub_pledge_id), (370, -2));
    // Clan.addNewSkill: a level up changes the row, a new skill inserts one.
    repo.update_clan_skill(900, 370, 2).await.unwrap();
    repo.insert_clan_skill(900, 611, 1, "Fire Squad", 100)
        .await
        .unwrap();
    assert_eq!(
        db.scalar("SELECT skill_level FROM clan_skills WHERE clan_id = 900 AND skill_id = 370")
            .await,
        2
    );
    assert_eq!(
        db.scalar("SELECT sub_pledge_id FROM clan_skills WHERE clan_id = 900 AND skill_id = 611")
            .await,
        100
    );

    // ClanTable.destroyClan
    repo.delete_clan(900, 0).await.unwrap();
    for table in [
        "clan_data",
        "clan_privs",
        "clan_skills",
        "clan_subpledges",
        "clan_notices",
    ] {
        assert_eq!(
            db.scalar(&format!("SELECT COUNT(*) FROM {table} WHERE clan_id = 900"))
                .await,
            0,
            "{table}"
        );
    }
    assert_eq!(db.scalar("SELECT COUNT(*) FROM clan_wars").await, 0);
    db.drop_database().await;
}

#[tokio::test]
async fn crests_and_wars_use_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let clans = ClanRepository::new(db.pool.clone());
    let crests = CrestRepository::new(db.pool.clone());
    for (clan_id, name, ally_id) in [(900, "Lions", 900), (901, "Tigers", 900), (902, "Bears", 0)] {
        clans
            .insert_clan(&NewClan {
                clan_id,
                clan_name: name.into(),
                clan_level: 5,
                castle_id: 0,
                blood_alliance_count: 0,
                blood_oath_count: 0,
                ally_id,
                ally_name: None,
                leader_id: 0,
                crest_id: 0,
                crest_large_id: 0,
                ally_crest_id: 0,
                new_leader_id: 0,
            })
            .await
            .unwrap();
    }

    // CrestTable.createCrest and load (highest id first), removeCrest
    crests.insert(1, &[1, 2, 3], 1).await.unwrap();
    crests.insert(7, &[0xDD; 300], 2).await.unwrap();
    crests.insert(3, &[9], 3).await.unwrap();
    let rows = crests.all().await.unwrap();
    assert_eq!(rows.iter().map(|r| r.crest_id).collect::<Vec<_>>(), [7, 3, 1]);
    assert_eq!(
        (rows[0].data.len(), rows[0].crest_type, rows[2].data.as_slice()),
        (300, 2, &[1u8, 2, 3][..])
    );
    // The primary key refuses an id that is taken (two servers picking the same next id).
    assert!(crests.insert(7, &[1], 1).await.is_err());
    crests.delete(3).await.unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM crests").await, 2);

    // Clan.changeClanCrest, changeLargeCrest, changeAllyCrest (both forms)
    clans.update_clan_crest(900, 1).await.unwrap();
    clans.update_clan_large_crest(900, 7).await.unwrap();
    clans.update_ally_crest_of_clan(902, 4).await.unwrap();
    clans.update_ally_crest_of_ally(900, 5).await.unwrap();
    assert_eq!(
        db.scalar("SELECT crest_id * 100 + crest_large_id FROM clan_data WHERE clan_id = 900")
            .await,
        107
    );
    // `WHERE ally_id = 900` reaches both clans of the alliance, not the clan outside it.
    assert_eq!(
        db.scalar("SELECT ally_crest_id FROM clan_data WHERE clan_id = 901")
            .await,
        5
    );
    assert_eq!(
        db.scalar("SELECT ally_crest_id FROM clan_data WHERE clan_id = 902")
            .await,
        4
    );

    // ClanTable.storeClanWars (REPLACE, ids as numbers into the VARCHAR columns) and
    // deleteClanWars (either direction)
    clans.store_clan_war(900, 902).await.unwrap();
    clans.store_clan_war(900, 902).await.unwrap();
    clans.store_clan_war(902, 900).await.unwrap();
    let wars = clans.clan_wars().await.unwrap();
    let mut pairs: Vec<_> = wars
        .iter()
        .map(|w| (w.clan1.as_str(), w.clan2.as_str()))
        .collect();
    pairs.sort_unstable();
    assert_eq!(pairs, [("900", "902"), ("902", "900")]);
    clans.delete_clan_war(902, 900).await.unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM clan_wars").await, 0);
    db.drop_database().await;
}
