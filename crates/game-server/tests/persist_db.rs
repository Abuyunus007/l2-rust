#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The persistence store against a throwaway MariaDB: store, restore, and the rows the Java
//! game server owns stay untouched (D-39).
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service,
//! e.g. `mysql://root@127.0.0.1:3306/`). Never point it at a server with real data: the test
//! only creates and drops a database named `l2_test_persist_*`, but it needs the rights to do
//! so.

use std::time::Duration;

use game_server::persist::{DbStore, LsNotice, PersistError, PlayerStore, RetryPolicy, spawn};
use game_server::runtime::OutboxMessage;
use l2_db::{
    CalendarDate, CharacterRepository, ItemLocation, ItemRepository, NewCharacter, NewItem, Pool,
    ShortcutRepository, SkillRepository, connect_with,
};
use l2_testkit::test_db::TestDb;
use l2_world::{ItemData, ObjectId, PersistCommand, PlayerData, ShortcutData, SkillData};
use sqlx::mysql::MySqlConnectOptions;

const SCHEMA: [&str; 4] = [
    include_str!("../../l2-db/sql/game/characters.sql"),
    include_str!("../../l2-db/sql/game/items.sql"),
    include_str!("../../l2-db/sql/game/character_skills.sql"),
    include_str!("../../l2-db/sql/game/character_shortcuts.sql"),
];

const CHAR_ID: i32 = 0x6000_0001;
const NOW: i64 = 1_700_000_000_000;

/// The login schema (from `TestDb`) plus the game tables the store writes, and a pool.
async fn setup() -> Option<(TestDb, Pool)> {
    let db = TestDb::create("persist").await?;
    for file in SCHEMA {
        let text: String = file
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        for statement in text.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            db.exec(statement).await;
        }
    }
    let (user, password, host, port) = db.parts();
    let mut options = MySqlConnectOptions::new()
        .host(&host)
        .port(port)
        .username(&user)
        .database(db.name());
    if !password.is_empty() {
        options = options.password(&password);
    }
    let pool = connect_with(options, 10).await.unwrap();
    Some((db, pool))
}

fn new_character() -> NewCharacter {
    NewCharacter {
        account_name: "acc".into(),
        char_id: CHAR_ID,
        char_name: "Hero".into(),
        level: 1,
        max_hp: 80,
        cur_hp: 40.0,
        max_cp: 32,
        cur_cp: 20.0,
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

fn item(id: i32, item_id: i32, count: i64, loc: &str, loc_data: i32) -> ItemData {
    ItemData {
        object_id: ObjectId(id),
        item_id,
        count,
        enchant_level: 0,
        loc: loc.into(),
        loc_data,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
    }
}

fn player_data() -> PlayerData {
    PlayerData {
        object_id: ObjectId(CHAR_ID),
        account: "acc".into(),
        name: "Hero".into(),
        title: String::new(),
        class_id: 53,
        base_class: 53,
        race: 4,
        sex: 1,
        face: 1,
        hair_style: 2,
        hair_color: 3,
        level: 5,
        exp: 1234,
        sp: 56,
        karma: 0,
        pvp_kills: 2,
        pk_kills: 1,
        access_level: 0,
        x: -71_338,
        y: 258_271,
        z: -3104,
        heading: 77,
        cur_hp: 60.0,
        max_hp: 90.0,
        cur_mp: 20.0,
        max_mp: 35.0,
        cur_cp: 10.0,
        max_cp: 40.0,
        online_time: 600,
        items: vec![
            item(0x6000_1001, 57, 500, "INVENTORY", 0),
            item(0x6000_1002, 1, 1, "PAPERDOLL", 7),
        ],
        skills: vec![SkillData {
            skill_id: 1216,
            skill_level: 2,
        }],
        shortcuts: vec![ShortcutData {
            slot: 0,
            page: 0,
            kind: 3,
            shortcut_id: 2,
            level: 0,
        }],
        profile: Default::default(),
    }
}

#[tokio::test]
async fn store_writes_the_snapshot_and_keeps_what_java_owns() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    let items = ItemRepository::new(pool.clone());
    let skills = SkillRepository::new(pool.clone());
    let shortcuts = ShortcutRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    // Columns M2 does not model, as the Java game server left them.
    db.exec(&format!(
        "UPDATE characters SET clanid = 77, nobless = 1, fame = 123, clan_privs = 9, \
         vitality_points = 555, pccafe_points = 42, language = 'en', bookmarkslot = 3 \
         WHERE charId = {CHAR_ID}"
    ))
    .await;
    // A row of this character that the snapshot does not carry (e.g. a warehouse item).
    items
        .insert_item(&NewItem {
            owner_id: CHAR_ID,
            item_id: 6,
            count: 1,
            loc: ItemLocation::Warehouse,
            loc_data: 0,
            enchant_level: 0,
            object_id: 0x6000_1003,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: -1,
            time: -1,
        })
        .await
        .unwrap();

    let mut store = DbStore::new(pool.clone());
    let mut data = player_data();
    store.store_player(&data).await.unwrap();

    let row = characters.restore_character(CHAR_ID).await.unwrap().unwrap();
    assert_eq!(row.level, Some(5));
    assert_eq!(
        (row.x, row.y, row.z, row.heading),
        (Some(-71_338), Some(258_271), Some(-3104), Some(77))
    );
    assert_eq!(row.exp, Some(1234));
    assert_eq!(row.sp, 56);
    assert_eq!((row.cur_hp, row.max_hp), (Some(60), Some(90)));
    assert_eq!(row.onlinetime, Some(600));
    assert_eq!(row.online, Some(1));
    assert_eq!(row.pvpkills, Some(2));
    // Not modelled: unchanged.
    assert_eq!(row.clanid, Some(77));
    assert_eq!(row.nobless, 1);
    assert_eq!(row.fame, 123);
    assert_eq!(row.clan_privs, Some(9));
    assert_eq!(row.vitality_points, 555);
    assert_eq!(row.pccafe_points, 42);
    assert_eq!(row.language.as_deref(), Some("en"));
    assert_eq!(row.bookmarkslot, 3);
    assert_eq!(row.title, None);

    let inventory = items
        .inventory_items(CHAR_ID, ItemLocation::Inventory, ItemLocation::Paperdoll)
        .await
        .unwrap();
    assert_eq!(inventory.len(), 2);
    assert_eq!(
        inventory
            .iter()
            .find(|i| i.object_id == 0x6000_1001)
            .unwrap()
            .count,
        500
    );
    // The warehouse item was not in the snapshot and is left alone.
    assert_eq!(
        items
            .items_of_location(CHAR_ID, ItemLocation::Warehouse)
            .await
            .unwrap()
            .len(),
        1
    );
    let learned = skills.skills_of(CHAR_ID, 0).await.unwrap();
    assert_eq!((learned[0].skill_id, learned[0].skill_level), (1216, 2));
    let bar = shortcuts.shortcuts_of(CHAR_ID, 0).await.unwrap();
    assert_eq!(bar.len(), 1);
    assert_eq!(bar[0].shortcut_id, Some(2));

    // Second store: a changed stack, a destroyed item, a new item, a levelled skill, a new
    // skill and a removed shortcut. Only the differences are written.
    data.items = vec![
        item(0x6000_1001, 57, 450, "INVENTORY", 0),
        item(0x6000_1004, 5, 3, "INVENTORY", 0),
    ];
    data.skills = vec![
        SkillData {
            skill_id: 1216,
            skill_level: 3,
        },
        SkillData {
            skill_id: 3,
            skill_level: 1,
        },
    ];
    data.shortcuts.clear();
    data.x = -1;
    store.store_player(&data).await.unwrap();

    let inventory = items
        .inventory_items(CHAR_ID, ItemLocation::Inventory, ItemLocation::Paperdoll)
        .await
        .unwrap();
    let ids: Vec<i32> = inventory.iter().map(|i| i.object_id).collect();
    assert!(ids.contains(&0x6000_1001) && ids.contains(&0x6000_1004) && !ids.contains(&0x6000_1002));
    assert_eq!(
        inventory
            .iter()
            .find(|i| i.object_id == 0x6000_1001)
            .unwrap()
            .count,
        450
    );
    assert_eq!(
        items
            .items_of_location(CHAR_ID, ItemLocation::Warehouse)
            .await
            .unwrap()
            .len(),
        1
    );
    let mut learned: Vec<(i32, i32)> = skills
        .skills_of(CHAR_ID, 0)
        .await
        .unwrap()
        .iter()
        .map(|s| (s.skill_id, s.skill_level))
        .collect();
    learned.sort_unstable();
    assert_eq!(learned, vec![(3, 1), (1216, 3)]);
    assert!(shortcuts.shortcuts_of(CHAR_ID, 0).await.unwrap().is_empty());
    assert_eq!(
        characters.restore_character(CHAR_ID).await.unwrap().unwrap().x,
        Some(-1)
    );

    // Leaving marks the character offline.
    store.player_offline(ObjectId(CHAR_ID)).await.unwrap();
    let row = characters.restore_character(CHAR_ID).await.unwrap().unwrap();
    assert_eq!(row.online, Some(0));
    assert!(row.last_access > 0);

    pool.close().await;
    db.drop_db().await;
}

#[tokio::test]
async fn a_store_for_the_wrong_account_or_a_missing_character_writes_nothing() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    let mut store = DbStore::new(pool.clone());

    let mut data = player_data();
    data.account = "someone-else".into();
    let err = store.store_player(&data).await.unwrap_err();
    assert!(matches!(err, PersistError::WrongAccount(CHAR_ID, _)), "{err}");
    let row = characters.restore_character(CHAR_ID).await.unwrap().unwrap();
    assert_eq!(row.level, Some(1));

    let mut data = player_data();
    data.object_id = ObjectId(CHAR_ID + 1);
    let err = store.store_player(&data).await.unwrap_err();
    assert!(matches!(err, PersistError::NoSuchCharacter(_)), "{err}");

    pool.close().await;
    db.drop_db().await;
}

#[tokio::test]
async fn the_task_stores_before_it_announces_the_logout() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let (notice_tx, mut notices) = tokio::sync::mpsc::unbounded_channel();
    let task = spawn(DbStore::new(pool.clone()), rx, notice_tx, RetryPolicy::default());
    tx.send(OutboxMessage::Persist(PersistCommand::StorePlayer(Box::new(
        player_data(),
    ))))
    .unwrap();
    tx.send(OutboxMessage::Persist(PersistCommand::PlayerLeft {
        account: "acc".into(),
        object_id: ObjectId(CHAR_ID),
        logout: true,
    }))
    .unwrap();

    let notice = tokio::time::timeout(Duration::from_secs(30), notices.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        notice,
        LsNotice::PlayerLogout {
            account: "acc".into()
        }
    );
    // Once the logout is announced, everything is in the database.
    let row = characters.restore_character(CHAR_ID).await.unwrap().unwrap();
    assert_eq!(row.level, Some(5));
    assert_eq!(row.online, Some(0));
    assert_eq!(
        ItemRepository::new(pool.clone())
            .items_of_location(CHAR_ID, ItemLocation::Inventory)
            .await
            .unwrap()
            .len(),
        1
    );

    drop(tx);
    task.await.unwrap();
    pool.close().await;
    db.drop_db().await;
}
