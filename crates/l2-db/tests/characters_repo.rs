#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration tests of `CharacterRepository` against a real MySQL/MariaDB server.
//!
//! They run only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the tests only create and drop databases named `l2_test_*`, but they need the rights
//! to do so. Each test gets a fresh database built from `sql/game/*.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{CalendarDate, CharacterRepository, CharacterUpdate, NewCharacter, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

/// `characters` plus every table `delete_character` touches.
const SCHEMA: [&str; 25] = [
    include_str!("../sql/game/characters.sql"),
    include_str!("../sql/game/character_contacts.sql"),
    include_str!("../sql/game/character_friends.sql"),
    include_str!("../sql/game/character_hennas.sql"),
    include_str!("../sql/game/character_macroses.sql"),
    include_str!("../sql/game/character_quests.sql"),
    include_str!("../sql/game/character_recipebook.sql"),
    include_str!("../sql/game/character_shortcuts.sql"),
    include_str!("../sql/game/character_skills.sql"),
    include_str!("../sql/game/character_skills_save.sql"),
    include_str!("../sql/game/character_subclasses.sql"),
    include_str!("../sql/game/heroes.sql"),
    include_str!("../sql/game/olympiad_nobles.sql"),
    include_str!("../sql/game/seven_signs.sql"),
    include_str!("../sql/game/pets.sql"),
    include_str!("../sql/game/item_attributes.sql"),
    include_str!("../sql/game/item_elementals.sql"),
    include_str!("../sql/game/item_variables.sql"),
    include_str!("../sql/game/items.sql"),
    include_str!("../sql/game/merchant_lease.sql"),
    include_str!("../sql/game/character_raid_points.sql"),
    include_str!("../sql/game/character_reco_bonus.sql"),
    include_str!("../sql/game/character_instance_time.sql"),
    include_str!("../sql/game/character_variables.sql"),
    include_str!("../sql/game/mods_wedding.sql"),
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

/// A throwaway database with the game schema.
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
            "l2_test_chars_{}_{}_{}",
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

    fn repo(&self) -> CharacterRepository {
        CharacterRepository::new(self.pool.clone())
    }

    async fn exec(&self, sql: String) {
        sqlx::query(AssertSqlSafe(sql)).execute(&self.pool).await.unwrap();
    }

    async fn count(&self, table: &str, column: &str, id: i32) -> i64 {
        sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {table} WHERE {column} = {id}"
        )))
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

const NOW: i64 = 1_700_000_000_000;
const ALICE: i32 = 268_500_001;
const BOB: i32 = 268_500_002;

fn new_character(char_id: i32, account: &str, name: &str) -> NewCharacter {
    NewCharacter {
        account_name: account.into(),
        char_id,
        char_name: name.into(),
        level: 1,
        max_hp: 80,
        cur_hp: 40.6,
        max_cp: 32,
        cur_cp: 20.4,
        max_mp: 30,
        cur_mp: 30.0,
        face: 1,
        hair_style: 2,
        hair_color: 3,
        sex: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        fame: 0,
        pvp_kills: 0,
        pk_kills: 0,
        clan_id: 0,
        race: 4,
        class_id: 53,
        delete_time: 0,
        can_craft: true,
        title: None,
        title_color: 0xECF9A2,
        access_level: 0,
        online: 0,
        is_in_7s_dungeon: false,
        clan_privs: 0,
        wants_peace: 0,
        base_class: 53,
        newbie: 1,
        noble: false,
        create_date: CalendarDate::from_unix_millis_utc(NOW),
        last_access: NOW,
    }
}

fn update_of(c: &NewCharacter) -> CharacterUpdate {
    CharacterUpdate {
        level: c.level,
        max_hp: c.max_hp,
        cur_hp: c.cur_hp,
        max_cp: c.max_cp,
        cur_cp: c.cur_cp,
        max_mp: c.max_mp,
        cur_mp: c.cur_mp,
        face: c.face,
        hair_style: c.hair_style,
        hair_color: c.hair_color,
        sex: c.sex,
        heading: 0,
        x: 1,
        y: 2,
        z: 3,
        exp: c.exp,
        exp_before_death: 0,
        sp: c.sp,
        karma: c.karma,
        fame: c.fame,
        pvp_kills: c.pvp_kills,
        pk_kills: c.pk_kills,
        clan_id: c.clan_id,
        race: c.race,
        class_id: c.class_id,
        delete_time: c.delete_time,
        title: c.title.clone(),
        title_color: c.title_color,
        access_level: c.access_level,
        online: c.online,
        is_in_7s_dungeon: c.is_in_7s_dungeon,
        clan_privs: c.clan_privs,
        wants_peace: c.wants_peace,
        base_class: c.base_class,
        online_time: 0,
        newbie: c.newbie,
        noble: c.noble,
        power_grade: 0,
        subpledge: 0,
        lvl_joined_academy: 0,
        apprentice: 0,
        sponsor: 0,
        clan_join_expiry_time: 0,
        clan_create_expiry_time: 0,
        char_name: c.char_name.clone(),
        death_penalty_level: 0,
        bookmark_slot: 0,
        vitality_points: 0,
        language: None,
        faction: 0,
        pccafe_points: 0,
    }
}

#[tokio::test]
async fn insert_then_restore_gives_what_java_would_store() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    assert_eq!(repo.restore_character(ALICE).await.unwrap(), None);

    let c = new_character(ALICE, "acc1", "Alice");
    repo.insert_character(&c).await.unwrap();
    let row = repo.restore_character(ALICE).await.unwrap().unwrap();

    assert_eq!(row.object_id(), ALICE);
    assert_eq!(row.account_name.as_deref(), Some("acc1"));
    assert_eq!(row.char_name, "Alice");
    assert_eq!(row.level, Some(1));
    assert_eq!(row.max_hp, Some(80));
    // Java binds HP/CP/MP as double; the server rounds into the integer column.
    assert_eq!(row.cur_hp, Some(41));
    assert_eq!(row.cur_cp, Some(20));
    assert_eq!(row.cur_mp, Some(30));
    assert_eq!(
        (row.face, row.hair_style, row.hair_color, row.sex),
        (Some(1), Some(2), Some(3), Some(1))
    );
    assert_eq!((row.race, row.classid, row.base_class), (Some(4), Some(53), 53));
    assert_eq!(row.cancraft, Some(1));
    assert_eq!(row.title, None);
    assert_eq!(row.title_color, 0xECF9A2);
    assert_eq!(row.newbie, Some(1));
    assert_eq!(row.nobless, 0);
    assert_eq!(row.deletetime, 0);
    assert_eq!(row.last_access, NOW as u64);
    // Written by the INSERT as literal 0, not the column default NULL.
    assert_eq!(row.power_grade, Some(0));
    // Not in the INSERT: they keep their column defaults.
    assert_eq!((row.heading, row.x, row.y, row.z), (None, None, None, None));
    assert_eq!(row.exp, Some(0));
    assert_eq!(row.exp_before_death, Some(0));
    assert_eq!(
        (row.transform_id, row.subpledge, row.faction, row.pccafe_points),
        (0, 0, 0, 0)
    );
    assert_eq!((row.language, row.onlinetime, row.char_slot), (None, None, None));

    // The DATE column keeps the date part only.
    let date: String = sqlx::query_scalar("SELECT CAST(createDate AS CHAR) FROM characters WHERE charId = ?")
        .bind(ALICE)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(date, "2023-11-14");

    // The primary key rejects a second insert of the same id.
    assert!(repo.insert_character(&c).await.is_err());

    db.drop_database().await;
}

#[tokio::test]
async fn update_character_writes_every_listed_column() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    let c = new_character(ALICE, "acc1", "Alice");
    repo.insert_character(&c).await.unwrap();
    repo.insert_character(&new_character(BOB, "acc2", "Bob"))
        .await
        .unwrap();

    let mut u = update_of(&c);
    u.level = 20;
    u.max_hp = 1500;
    u.cur_hp = 1499.5;
    u.heading = 32768;
    u.x = -71_301;
    u.y = 258_259;
    u.z = -3_132;
    u.exp = 4_200_000_000;
    u.exp_before_death = 4_100_000_000;
    u.sp = 123_456;
    u.karma = 5;
    u.fame = 7;
    u.pvp_kills = 3;
    u.pk_kills = 1;
    u.clan_id = 42;
    u.delete_time = 99;
    u.title = Some("Hero".into());
    u.title_color = 0xFF00FF;
    u.access_level = 8;
    u.online = 1;
    u.is_in_7s_dungeon = true;
    u.clan_privs = 0x1f;
    u.wants_peace = 1;
    u.online_time = 3600;
    u.newbie = 0;
    u.noble = true;
    u.power_grade = 5;
    u.subpledge = -1;
    u.lvl_joined_academy = 9;
    u.apprentice = 11;
    u.sponsor = 12;
    u.clan_join_expiry_time = 111;
    u.clan_create_expiry_time = 222;
    u.char_name = "Alicia".into();
    u.death_penalty_level = 2;
    u.bookmark_slot = 3;
    u.vitality_points = 20_000;
    u.language = Some("en".into());
    u.faction = 2;
    u.pccafe_points = -4;
    repo.update_character(ALICE, &u).await.unwrap();

    let row = repo.restore_character(ALICE).await.unwrap().unwrap();
    assert_eq!(row.char_name, "Alicia");
    assert_eq!(
        (row.level, row.max_hp, row.cur_hp),
        (Some(20), Some(1500), Some(1500))
    );
    assert_eq!(
        (row.heading, row.x, row.y, row.z),
        (Some(32768), Some(-71_301), Some(258_259), Some(-3_132))
    );
    assert_eq!(
        (row.exp, row.exp_before_death, row.sp),
        (Some(4_200_000_000), Some(4_100_000_000), 123_456)
    );
    assert_eq!(
        (row.karma, row.fame, row.pvpkills, row.pkkills),
        (Some(5), 7, Some(3), Some(1))
    );
    assert_eq!((row.clanid, row.deletetime), (Some(42), 99));
    assert_eq!(row.title.as_deref(), Some("Hero"));
    assert_eq!(
        (row.title_color, row.accesslevel, row.online),
        (0xFF00FF, Some(8), Some(1))
    );
    assert_eq!(
        (row.isin7sdungeon, row.clan_privs, row.wantspeace),
        (1, Some(0x1f), Some(1))
    );
    assert_eq!(
        (row.onlinetime, row.newbie, row.nobless),
        (Some(3600), Some(0), 1)
    );
    assert_eq!(
        (row.power_grade, row.subpledge, row.lvl_joined_academy),
        (Some(5), -1, 9)
    );
    assert_eq!((row.apprentice, row.sponsor), (11, 12));
    assert_eq!(
        (row.clan_join_expiry_time, row.clan_create_expiry_time),
        (111, 222)
    );
    assert_eq!(
        (row.death_penalty_level, row.bookmarkslot, row.vitality_points),
        (2, 3, 20_000)
    );
    assert_eq!(
        (row.language.as_deref(), row.faction, row.pccafe_points),
        (Some("en"), 2, -4)
    );
    // Not part of the UPDATE: account, create date, last access, cancraft stay as inserted.
    assert_eq!(row.account_name.as_deref(), Some("acc1"));
    assert_eq!(row.last_access, NOW as u64);
    assert_eq!(row.cancraft, Some(1));

    // The WHERE clause touches only that character.
    let bob = repo.restore_character(BOB).await.unwrap().unwrap();
    assert_eq!((bob.char_name.as_str(), bob.level, bob.x), ("Bob", Some(1), None));

    db.drop_database().await;
}

#[tokio::test]
async fn online_status_and_startup_reset() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    repo.insert_character(&new_character(ALICE, "acc1", "Alice"))
        .await
        .unwrap();
    repo.insert_character(&new_character(BOB, "acc1", "Bob"))
        .await
        .unwrap();

    repo.update_online_status(ALICE, 1, NOW + 1000).await.unwrap();
    let alice = repo.restore_character(ALICE).await.unwrap().unwrap();
    assert_eq!((alice.online, alice.last_access), (Some(1), (NOW + 1000) as u64));
    let bob = repo.restore_character(BOB).await.unwrap().unwrap();
    assert_eq!((bob.online, bob.last_access), (Some(0), NOW as u64));

    repo.update_online_status(BOB, 1, NOW + 2000).await.unwrap();
    repo.reset_all_online().await.unwrap();
    for id in [ALICE, BOB] {
        let row = repo.restore_character(id).await.unwrap().unwrap();
        assert_eq!(row.online, Some(0));
    }
    // lastAccess is left alone by the reset.
    let bob = repo.restore_character(BOB).await.unwrap().unwrap();
    assert_eq!(bob.last_access, (NOW + 2000) as u64);

    db.drop_database().await;
}

#[tokio::test]
async fn character_selection_lists_the_account_oldest_first() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    let mut newest = new_character(ALICE, "acc1", "Newest");
    newest.create_date = CalendarDate {
        year: 2024,
        month: 5,
        day: 1,
    };
    let mut oldest = new_character(BOB, "acc1", "Oldest");
    oldest.create_date = CalendarDate {
        year: 2020,
        month: 1,
        day: 31,
    };
    let mut other = new_character(ALICE + 10, "acc2", "Other");
    other.create_date = CalendarDate {
        year: 2010,
        month: 1,
        day: 1,
    };
    for c in [&newest, &oldest, &other] {
        repo.insert_character(c).await.unwrap();
    }

    let names: Vec<_> = repo
        .characters_of_account("acc1")
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.char_name)
        .collect();
    assert_eq!(names, ["Oldest", "Newest"]);
    assert!(repo.characters_of_account("nobody").await.unwrap().is_empty());

    // The other characters of the account, without the one that logs in.
    let others = repo.other_characters("acc1", ALICE).await.unwrap();
    assert_eq!(others.len(), 1);
    assert_eq!(
        (others[0].char_id, others[0].char_name.as_str()),
        (BOB as u32, "Oldest")
    );

    assert_eq!(repo.account_character_count("acc1").await.unwrap(), 2);
    assert_eq!(repo.account_character_count("acc2").await.unwrap(), 1);
    assert_eq!(repo.account_character_count("nobody").await.unwrap(), 0);

    db.drop_database().await;
}

#[tokio::test]
async fn delete_marks_clan_lookup_and_restore() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    let mut member = new_character(ALICE, "acc1", "Member");
    member.clan_id = 77;
    repo.insert_character(&member).await.unwrap();
    repo.insert_character(&new_character(BOB, "acc1", "Loner"))
        .await
        .unwrap();

    assert_eq!(repo.clan_id_of(ALICE).await.unwrap(), 77);
    assert_eq!(repo.clan_id_of(BOB).await.unwrap(), 0);
    assert_eq!(repo.clan_id_of(12345).await.unwrap(), 0, "missing row reads as 0");

    assert_eq!(repo.delete_times_of_account("acc1").await.unwrap(), [0, 0]);
    repo.set_delete_time(BOB, NOW + 86_400_000).await.unwrap();
    let mut times = repo.delete_times_of_account("acc1").await.unwrap();
    times.sort_unstable();
    assert_eq!(times, [0, (NOW + 86_400_000) as u64]);
    assert_eq!(
        repo.restore_character(BOB).await.unwrap().unwrap().deletetime,
        (NOW + 86_400_000) as u64
    );

    repo.clear_delete_time(BOB).await.unwrap();
    assert_eq!(repo.delete_times_of_account("acc1").await.unwrap(), [0, 0]);

    db.drop_database().await;
}

/// `(insert statement, table, column)`; `{id}` is the character id and `{item}` its item id.
/// Every table `deleteCharByObjId` touches, in Java's order.
const OWNED_ROWS: [(&str, &str, &str); 21] = [
    (
        "INSERT INTO character_contacts (charId, contactId) VALUES ({id}, 5)",
        "character_contacts",
        "charId",
    ),
    (
        "INSERT INTO character_friends (charId, friendId) VALUES ({id}, 5)",
        "character_friends",
        "charId",
    ),
    (
        "INSERT INTO character_hennas (charId, symbol_id) VALUES ({id}, 1)",
        "character_hennas",
        "charId",
    ),
    (
        "INSERT INTO character_macroses (charId, id) VALUES ({id}, 1)",
        "character_macroses",
        "charId",
    ),
    (
        "INSERT INTO character_quests (charId, name) VALUES ({id}, 'q')",
        "character_quests",
        "charId",
    ),
    (
        "INSERT INTO character_recipebook (charId, id) VALUES ({id}, 1)",
        "character_recipebook",
        "charId",
    ),
    (
        "INSERT INTO character_shortcuts (charId, slot) VALUES ({id}, 1)",
        "character_shortcuts",
        "charId",
    ),
    (
        "INSERT INTO character_skills (charId, skill_id) VALUES ({id}, 1)",
        "character_skills",
        "charId",
    ),
    (
        "INSERT INTO character_skills_save (charId, skill_id) VALUES ({id}, 1)",
        "character_skills_save",
        "charId",
    ),
    (
        "INSERT INTO character_subclasses (charId, class_id) VALUES ({id}, 1)",
        "character_subclasses",
        "charId",
    ),
    ("INSERT INTO heroes (charId) VALUES ({id})", "heroes", "charId"),
    (
        "INSERT INTO olympiad_nobles (charId) VALUES ({id})",
        "olympiad_nobles",
        "charId",
    ),
    (
        "INSERT INTO seven_signs (charId) VALUES ({id})",
        "seven_signs",
        "charId",
    ),
    (
        "INSERT INTO pets (item_obj_id, level) VALUES ({item}, 1)",
        "pets",
        "item_obj_id",
    ),
    (
        "INSERT INTO item_attributes (itemId) VALUES ({item})",
        "item_attributes",
        "itemId",
    ),
    (
        "INSERT INTO items (owner_id, object_id) VALUES ({id}, {item})",
        "items",
        "owner_id",
    ),
    (
        "INSERT INTO merchant_lease (merchant_id, player_id) VALUES (1, {id})",
        "merchant_lease",
        "player_id",
    ),
    (
        "INSERT INTO character_raid_points (charId) VALUES ({id})",
        "character_raid_points",
        "charId",
    ),
    (
        "INSERT INTO character_reco_bonus (charId) VALUES ({id})",
        "character_reco_bonus",
        "charId",
    ),
    (
        "INSERT INTO character_instance_time (charId) VALUES ({id})",
        "character_instance_time",
        "charId",
    ),
    (
        "INSERT INTO character_variables (charId, var, val) VALUES ({id}, 'v', 'x')",
        "character_variables",
        "charId",
    ),
];

async fn give_rows(db: &TestDb, id: i32) {
    let item = id + 1_000;
    for (template, _, _) in OWNED_ROWS {
        db.exec(
            template
                .replace("{id}", &id.to_string())
                .replace("{item}", &item.to_string()),
        )
        .await;
    }
}

/// Row count of `table` for a character: the pets and item_attributes rows hang off the item.
async fn rows_of(db: &TestDb, table: &str, column: &str, id: i32) -> i64 {
    let key = if column == "item_obj_id" || column == "itemId" {
        id + 1_000
    } else {
        id
    };
    db.count(table, column, key).await
}

#[tokio::test]
async fn delete_character_removes_everything_of_that_character_only() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    repo.insert_character(&new_character(ALICE, "acc1", "Alice"))
        .await
        .unwrap();
    repo.insert_character(&new_character(BOB, "acc1", "Bob"))
        .await
        .unwrap();
    give_rows(&db, ALICE).await;
    give_rows(&db, BOB).await;
    // Rows that reference the character from the other side of a two-column statement.
    db.exec(format!(
        "INSERT INTO character_contacts (charId, contactId) VALUES ({BOB}, {ALICE})"
    ))
    .await;
    db.exec(format!(
        "INSERT INTO character_friends (charId, friendId) VALUES ({BOB}, {ALICE})"
    ))
    .await;
    db.exec(format!(
        "INSERT INTO mods_wedding (player1Id, player2Id) VALUES ({BOB}, {ALICE})"
    ))
    .await;

    for (_, table, column) in OWNED_ROWS {
        assert!(rows_of(&db, table, column, ALICE).await >= 1, "{table} before");
    }

    // The wedding mod is off: mods_wedding stays.
    repo.delete_character(ALICE, false).await.unwrap();

    assert_eq!(db.count("characters", "charId", ALICE).await, 0);
    for (_, table, column) in OWNED_ROWS {
        assert_eq!(rows_of(&db, table, column, ALICE).await, 0, "{table} after");
    }
    // Contacts and friends are removed from the other side too.
    assert_eq!(db.count("character_contacts", "contactId", ALICE).await, 0);
    assert_eq!(db.count("character_friends", "friendId", ALICE).await, 0);
    assert_eq!(db.count("mods_wedding", "player2Id", ALICE).await, 1);

    // Bob keeps everything of his own (his own contact/friend rows with id 5 survive).
    assert_eq!(db.count("characters", "charId", BOB).await, 1);
    for (_, table, column) in OWNED_ROWS {
        assert!(rows_of(&db, table, column, BOB).await >= 1, "{table} of bob");
    }

    // With the wedding mod on, the marriage row goes.
    repo.delete_character(ALICE, true).await.unwrap();
    assert_eq!(db.count("mods_wedding", "player2Id", ALICE).await, 0);
    repo.delete_character(BOB, true).await.unwrap();
    assert_eq!(db.count("characters", "charId", BOB).await, 0);

    // A negative id and an unknown id are no-ops.
    repo.delete_character(-5, true).await.unwrap();
    repo.delete_character(999_999, true).await.unwrap();

    db.drop_database().await;
}

#[tokio::test]
async fn name_lookups() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    let mut gm = new_character(ALICE, "acc1", "Alice");
    gm.access_level = 100;
    repo.insert_character(&gm).await.unwrap();
    repo.insert_character(&new_character(BOB, "acc2", "Bob"))
        .await
        .unwrap();

    let mut all = repo.all_names().await.unwrap();
    all.sort_by_key(|r| r.char_id);
    let summary: Vec<_> = all
        .iter()
        .map(|r| (r.char_id, r.char_name.as_str(), r.accesslevel))
        .collect();
    assert_eq!(
        summary,
        [(ALICE as u32, "Alice", Some(100)), (BOB as u32, "Bob", Some(0))]
    );

    assert_eq!(repo.id_by_name("Alice").await.unwrap(), Some((ALICE as u32, 100)));
    // The collation is case-insensitive, so Java finds the character with any case.
    assert_eq!(repo.id_by_name("aLICE").await.unwrap(), Some((ALICE as u32, 100)));
    assert_eq!(repo.id_by_name("Nobody").await.unwrap(), None);

    assert_eq!(repo.name_by_id(BOB).await.unwrap(), Some(("Bob".into(), 0)));
    assert_eq!(repo.name_by_id(1).await.unwrap(), None);

    assert!(repo.name_exists("Bob").await.unwrap());
    assert!(repo.name_exists("bob").await.unwrap());
    assert!(!repo.name_exists("Bobby").await.unwrap());

    repo.update_access_level(BOB, -100).await.unwrap();
    assert_eq!(repo.name_by_id(BOB).await.unwrap(), Some(("Bob".into(), -100)));
    assert_eq!(repo.name_by_id(ALICE).await.unwrap(), Some(("Alice".into(), 100)));

    db.drop_database().await;
}
