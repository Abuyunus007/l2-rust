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
    CalendarDate, CharacterRepository, ItemLocation, ItemRepository, ItemReuseRepository, NewCharacter,
    NewItem, Pool, RecommendationRepository, ShortcutRepository, SkillRepository, VariableRepository,
    connect_with,
};
use l2_testkit::test_db::TestDb;
use l2_world::{
    ItemData, ItemRowOp, ObjectId, PersistCommand, PlayerData, SavedEffect, SavedItemReuse, ShortcutData,
    SkillData,
};
use sqlx::mysql::MySqlConnectOptions;

const SCHEMA: &[&str] = l2_db::GAME_SCHEMA;

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
        ..Default::default()
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
    // Java writes `getAppearance().getTitleColor()`, which the PvP title colours change (M6-PVP-01).
    data.profile.title_color = 0x009C_9C9C;
    // Java writes `getFame()`, which multisell fame ingredients and products change.
    data.profile.fame = 321;
    store.store_player(&data).await.unwrap();

    let row = characters.restore_character(CHAR_ID).await.unwrap().unwrap();
    assert_eq!(row.title_color, 0x009C_9C9C);
    assert_eq!(row.fame, 321);
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

/// A level up (M4-EXP-01): the skills it teaches are written at once like Java's `storeSkill`
/// (`REPLACE` for a new skill, `UPDATE` of the level for a known one), a skill it takes is
/// deleted, and the next full store agrees. `expBeforeDeath` is written from the snapshot.
/// M5-SAVE-01: `Player.storeEffect` replaces the character's `character_skills_save` rows with
/// the saved buffs and reuse timers, and `restoreEffects`' read (`skill_effects_of`) gets them back
/// in order. A snapshot that carries no effects (`effects_to_store` None) leaves the rows alone.
#[tokio::test]
async fn saved_buffs_are_written_like_java_and_read_back_in_order() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    let skills = SkillRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    let mut store = DbStore::new(pool.clone());
    let saved = |skill_id, skill_level, remaining_time, restore_type| SavedEffect {
        skill_id,
        skill_level,
        remaining_time,
        reuse_delay: if restore_type == 1 { 60_000 } else { 0 },
        systime: if restore_type == 1 { NOW + 60_000 } else { 0 },
        restore_type,
    };
    let shape = |rows: Vec<l2_db::SkillSaveRow>| {
        rows.iter()
            .map(|r| (r.skill_id, r.skill_level, r.remaining_time, r.restore_type))
            .collect::<Vec<_>>()
    };

    let mut data = player_data();
    data.items = vec![];
    data.shortcuts = vec![];
    data.skills = vec![];
    data.profile.effects_to_store = Some(vec![saved(9001, 1, 50, 0), saved(1040, 3, -1, 1)]);
    store.store_player(&data).await.unwrap();
    assert_eq!(
        shape(skills.skill_effects_of(CHAR_ID, 0).await.unwrap()),
        vec![(9001, 1, 50, 0), (1040, 3, -1, 1)]
    );
    let reuse = skills.skill_effects_of(CHAR_ID, 0).await.unwrap();
    assert_eq!(
        (reuse[1].reuse_delay, reuse[1].systime),
        (60_000, (NOW + 60_000) as u64)
    );

    // The next store replaces the rows (DELETE, then the new rows).
    data.profile.effects_to_store = Some(vec![saved(9002, 1, 30, 0)]);
    store.store_player(&data).await.unwrap();
    assert_eq!(
        shape(skills.skill_effects_of(CHAR_ID, 0).await.unwrap()),
        vec![(9002, 1, 30, 0)]
    );

    // No effects in the snapshot: the table is not touched.
    data.profile.effects_to_store = None;
    store.store_player(&data).await.unwrap();
    assert_eq!(
        shape(skills.skill_effects_of(CHAR_ID, 0).await.unwrap()),
        vec![(9002, 1, 30, 0)]
    );

    // An empty list clears the rows, like Java's DELETE with no batch.
    data.profile.effects_to_store = Some(vec![]);
    store.store_player(&data).await.unwrap();
    assert!(skills.skill_effects_of(CHAR_ID, 0).await.unwrap().is_empty());

    pool.close().await;
    db.drop_db().await;
}

/// M5-FIX-03: `Player.storeItemReuseDelay` replaces the character's `character_item_reuse_save`
/// rows with the timers that still run (DELETE, then the batch), and `restoreItemReuse`' read
/// (`item_reuse_of`) gets them back. A snapshot without timers (`None`) leaves the rows alone.
#[tokio::test]
async fn item_reuse_timers_are_written_like_java_and_read_back() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    let reuse = ItemReuseRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    let mut store = DbStore::new(pool.clone());
    let timer = |item_id, item_obj_id| SavedItemReuse {
        item_id,
        item_obj_id,
        reuse_delay: 10_000,
        systime: NOW + 10_000,
    };
    let shape = |mut rows: Vec<l2_db::ItemReuseRow>| {
        rows.sort_by_key(|r| r.item_obj_id);
        rows.iter()
            .map(|r| (r.item_id, r.item_obj_id, i64::from(r.reuse_delay), r.systime))
            .collect::<Vec<_>>()
    };

    let mut data = player_data();
    data.items = vec![];
    data.shortcuts = vec![];
    data.skills = vec![];
    data.profile.item_reuse_to_store = Some(vec![timer(728, 101), timer(1060, 102)]);
    store.store_player(&data).await.unwrap();
    assert_eq!(
        shape(reuse.item_reuse_of(CHAR_ID).await.unwrap()),
        vec![
            (728, 101, 10_000, (NOW + 10_000) as u64),
            (1060, 102, 10_000, (NOW + 10_000) as u64)
        ]
    );

    // The next store replaces the rows.
    data.profile.item_reuse_to_store = Some(vec![timer(728, 101)]);
    store.store_player(&data).await.unwrap();
    assert_eq!(shape(reuse.item_reuse_of(CHAR_ID).await.unwrap()).len(), 1);

    // No timers in the snapshot: the table is not touched.
    data.profile.item_reuse_to_store = None;
    store.store_player(&data).await.unwrap();
    assert_eq!(shape(reuse.item_reuse_of(CHAR_ID).await.unwrap()).len(), 1);

    // An empty list clears the rows, like Java's DELETE with no batch.
    data.profile.item_reuse_to_store = Some(vec![]);
    store.store_player(&data).await.unwrap();
    assert!(reuse.item_reuse_of(CHAR_ID).await.unwrap().is_empty());

    pool.close().await;
    db.drop_db().await;
}

#[tokio::test]
async fn skill_changes_and_exp_before_death_are_written_like_java() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    let skills = SkillRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    let owner = ObjectId(CHAR_ID);
    let mut store = DbStore::new(pool.clone());
    let skill = |skill_id, skill_level| SkillData {
        skill_id,
        skill_level,
    };
    let learned = |rows: Vec<l2_db::SkillLevel>| {
        let mut rows: Vec<(i32, i32)> = rows.iter().map(|s| (s.skill_id, s.skill_level)).collect();
        rows.sort_unstable();
        rows
    };

    store
        .store_skill_changes(owner, &[skill(239, 1), skill(1216, 1)], &[])
        .await
        .unwrap();
    assert_eq!(
        learned(skills.skills_of(CHAR_ID, 0).await.unwrap()),
        vec![(239, 1), (1216, 1)]
    );
    // The next level raises one and takes the other.
    store
        .store_skill_changes(owner, &[skill(239, 2)], &[1216])
        .await
        .unwrap();
    assert_eq!(
        learned(skills.skills_of(CHAR_ID, 0).await.unwrap()),
        vec![(239, 2)]
    );

    // The full store of the same state writes nothing different, and carries the exp column.
    let mut data = player_data();
    data.items = vec![];
    data.shortcuts = vec![];
    data.skills = vec![skill(239, 2)];
    data.profile.exp_before_death = 987_654;
    store.store_player(&data).await.unwrap();
    assert_eq!(
        learned(skills.skills_of(CHAR_ID, 0).await.unwrap()),
        vec![(239, 2)]
    );
    let row = characters.restore_character(CHAR_ID).await.unwrap().unwrap();
    assert_eq!(row.exp_before_death, Some(987_654));
    assert_eq!((row.level, row.exp, row.sp), (Some(5), Some(1234), 56));

    pool.close().await;
    db.drop_db().await;
}

/// M4-VIT-01: the vitality column is the whole number of the points, the Nevit variables are
/// upserted and deleted by name like `PlayerVariables.saveNowSync`, and `character_reco_bonus`
/// is replaced like `Player.storeRecommendations`. Rows of other variables stay.
#[tokio::test]
async fn vitality_nevit_variables_and_the_hourglass_are_written_like_java() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    let variables = VariableRepository::new(pool.clone());
    let reco = RecommendationRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    // A variable the Java game server owns, and one the world will remove.
    variables
        .upsert_variable(CHAR_ID, "UI_KEY_MAPPING", "1\t2")
        .await
        .unwrap();
    variables
        .upsert_variable(CHAR_ID, "nevit_b", "100")
        .await
        .unwrap();

    let mut store = DbStore::new(pool.clone());
    let mut data = player_data();
    data.items = vec![];
    data.shortcuts = vec![];
    data.skills = vec![];
    data.profile.vitality_points = 14_962;
    data.profile.reco_store = true;
    data.profile.recom_have = 3;
    data.profile.recom_left = 17;
    data.profile.recom_time_left_ms = 3_590_000;
    for (name, value) in [("hunting_points", "72"), ("hunting_time", "30")] {
        data.profile.variables.insert(name.into(), value.into());
        data.profile.variables_changed.insert(name.into());
    }
    data.profile.variables_removed.insert("nevit_b".into());
    store.store_player(&data).await.unwrap();

    let row = characters.restore_character(CHAR_ID).await.unwrap().unwrap();
    assert_eq!(row.vitality_points, 14_962);
    let bonus = reco.reco_bonus_of(CHAR_ID).await.unwrap().unwrap();
    assert_eq!(
        (bonus.rec_have, bonus.rec_left, bonus.time_left),
        (3, 17, 3_590_000)
    );
    let mut stored = variables.variables_of(CHAR_ID).await.unwrap();
    stored.sort();
    assert_eq!(
        stored,
        vec![
            ("UI_KEY_MAPPING".to_owned(), "1\t2".to_owned()),
            ("hunting_points".to_owned(), "72".to_owned()),
            ("hunting_time".to_owned(), "30".to_owned()),
        ]
    );

    // A second store with a new value and the same sets is the same upsert, not a second row.
    data.profile
        .variables
        .insert("hunting_points".into(), "144".into());
    data.profile.recom_time_left_ms = 3_000_000;
    store.store_player(&data).await.unwrap();
    let stored = variables.variables_of(CHAR_ID).await.unwrap();
    assert_eq!(stored.len(), 3);
    assert!(stored.contains(&("hunting_points".to_owned(), "144".to_owned())));
    assert_eq!(
        reco.reco_bonus_of(CHAR_ID).await.unwrap().unwrap().time_left,
        3_000_000
    );

    // A snapshot that does not carry the vitality (0) and does not track the recommendations
    // leaves the column and the row as they are.
    let mut plain = player_data();
    plain.items = vec![];
    plain.shortcuts = vec![];
    plain.skills = vec![];
    store.store_player(&plain).await.unwrap();
    let row = characters.restore_character(CHAR_ID).await.unwrap().unwrap();
    assert_eq!(row.vitality_points, 14_962);
    assert_eq!(
        reco.reco_bonus_of(CHAR_ID).await.unwrap().unwrap().time_left,
        3_000_000
    );

    pool.close().await;
    db.drop_db().await;
}

#[tokio::test]
async fn item_changes_are_written_at_once_with_javas_columns() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    let items = ItemRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    // What the Java game server left: a sword in the bag and a warehouse item.
    for (object_id, item_id, loc) in [
        (0x6000_1001, 2, ItemLocation::Inventory),
        (0x6000_1003, 6, ItemLocation::Warehouse),
    ] {
        items
            .insert_item(&NewItem {
                owner_id: CHAR_ID,
                item_id,
                count: 1,
                loc,
                loc_data: 0,
                enchant_level: 0,
                object_id,
                custom_type1: 0,
                custom_type2: 0,
                mana_left: -1,
                time: -1,
            })
            .await
            .unwrap();
    }
    let owner = ObjectId(CHAR_ID);
    let mut store = DbStore::new(pool.clone());

    // Equip: Java's `Item.updateDatabase()` -> UPDATE items SET owner_id, count, loc, loc_data, ...
    let sword = item(0x6000_1001, 2, 1, "PAPERDOLL", 5);
    store.store_item_changes(owner, &[sword], &[]).await.unwrap();
    let worn = items
        .inventory_items(CHAR_ID, ItemLocation::Inventory, ItemLocation::Paperdoll)
        .await
        .unwrap();
    assert_eq!(worn.len(), 1);
    assert_eq!(worn[0].loc.as_deref(), Some("PAPERDOLL"));
    assert_eq!(worn[0].loc_data, Some(5));

    // Unequip, and a row Rust created (an INSERT, the id is not known to the store).
    let back = item(0x6000_1001, 2, 1, "INVENTORY", 0);
    let created = item(0x6000_1010, 57, 300, "INVENTORY", 0);
    store
        .store_item_changes(owner, &[back.clone(), created.clone()], &[])
        .await
        .unwrap();
    let bag = items
        .inventory_items(CHAR_ID, ItemLocation::Inventory, ItemLocation::Paperdoll)
        .await
        .unwrap();
    assert_eq!(bag.len(), 2);
    assert!(bag.iter().all(|i| i.loc.as_deref() == Some("INVENTORY")));
    assert_eq!(
        bag.iter().find(|i| i.object_id == 0x6000_1010).unwrap().count,
        300
    );

    // Destroy: the row goes (`removeFromDb`); an id nobody knows is ignored.
    store
        .store_item_changes(owner, &[], &[ObjectId(0x6000_1010), ObjectId(0x6000_FFFF)])
        .await
        .unwrap();
    let bag = items
        .inventory_items(CHAR_ID, ItemLocation::Inventory, ItemLocation::Paperdoll)
        .await
        .unwrap();
    assert_eq!(bag.len(), 1);

    // The next full store agrees with what was written: no duplicate insert, no extra delete,
    // and the warehouse item stays.
    let mut data = player_data();
    data.items = vec![back];
    store.store_player(&data).await.unwrap();
    let bag = items
        .inventory_items(CHAR_ID, ItemLocation::Inventory, ItemLocation::Paperdoll)
        .await
        .unwrap();
    assert_eq!(bag.len(), 1);
    assert_eq!(
        items
            .items_of_location(CHAR_ID, ItemLocation::Warehouse)
            .await
            .unwrap()
            .len(),
        1
    );

    pool.close().await;
    db.drop_db().await;
}

/// M6-TRADE-01: a traded item changes owner (`Item.updateDatabase(true)` after `setOwnerId`).
/// The world sends the row leaving the old owner, then the whole row to the new one; neither
/// character's later full store may delete or duplicate it.
#[tokio::test]
async fn an_item_handed_to_another_character_changes_owner_and_survives_both_stores() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    const MATE: i32 = CHAR_ID + 1;
    let characters = CharacterRepository::new(pool.clone());
    let items = ItemRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    let mut mate = new_character();
    mate.char_id = MATE;
    mate.char_name = "Mate".into();
    characters.insert_character(&mate).await.unwrap();
    let mut store = DbStore::new(pool.clone());

    // The giver stores its bag once, so the store knows the row.
    let sword = item(0x6000_1001, 2, 1, "INVENTORY", 0);
    let mut data = player_data();
    data.items = vec![sword.clone()];
    store.store_player(&data).await.unwrap();

    // The hand-over, in the order the world sends it.
    store
        .store_item_changes(ObjectId(CHAR_ID), &[], &[sword.object_id])
        .await
        .unwrap();
    store
        .store_item_changes(ObjectId(MATE), std::slice::from_ref(&sword), &[])
        .await
        .unwrap();
    let bag_of = |owner: i32| {
        let items = items.clone();
        async move {
            items
                .inventory_items(owner, ItemLocation::Inventory, ItemLocation::Paperdoll)
                .await
                .unwrap()
        }
    };
    assert!(bag_of(CHAR_ID).await.is_empty());
    let received = bag_of(MATE).await;
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].object_id, 0x6000_1001);
    assert_eq!(received[0].item_id, Some(2));

    // The giver's next full store has no sword: the row is not Mate's to lose.
    data.items = vec![];
    store.store_player(&data).await.unwrap();
    assert_eq!(bag_of(MATE).await.len(), 1);
    assert!(bag_of(CHAR_ID).await.is_empty());

    pool.close().await;
    db.drop_db().await;
}

/// M6-WH-01: items moved between the inventory, the warehouses and the freight are written with
/// Java's columns (`owner_id`, `loc`, `count`), the clan warehouse belongs to the clan, and the
/// character's full stores neither delete a deposited item nor insert a withdrawn one again.
#[tokio::test]
async fn warehouse_moves_write_the_rows_and_survive_full_stores() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    const MATE: i32 = CHAR_ID + 1;
    const CLAN: i32 = 500;
    let characters = CharacterRepository::new(pool.clone());
    let items = ItemRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    let mut mate = new_character();
    mate.char_id = MATE;
    mate.char_name = "Mate".into();
    characters.insert_character(&mate).await.unwrap();
    let mut store = DbStore::new(pool.clone());
    let me = ObjectId(CHAR_ID);
    let at = |owner: i32, loc: ItemLocation| {
        let items = items.clone();
        async move {
            let mut rows = items.items_of_location(owner, loc).await.unwrap();
            rows.sort_by_key(|r| r.object_id);
            rows.into_iter()
                .map(|r| (r.object_id, r.count))
                .collect::<Vec<_>>()
        }
    };

    // The bag was stored once: the sword and a pile of 500.
    let sword = item(0x6000_1001, 2, 1, "INVENTORY", 0);
    let pile = item(0x6000_1003, 23, 500, "INVENTORY", 0);
    let mut data = player_data();
    data.items = vec![sword.clone(), pile.clone()];
    store.store_player(&data).await.unwrap();

    // Deposit: the sword moves whole, 100 of the pile go to a new row.
    let mut deposited = sword.clone();
    deposited.loc = "WAREHOUSE".into();
    let mut pile_left = pile.clone();
    pile_left.count = 400;
    let new_pile = item(0x6000_1004, 23, 100, "WAREHOUSE", 0);
    store
        .store_item_rows(
            Some(me),
            &[
                ItemRowOp::Write {
                    owner_id: CHAR_ID,
                    item: deposited.clone(),
                },
                ItemRowOp::Write {
                    owner_id: CHAR_ID,
                    item: pile_left.clone(),
                },
                ItemRowOp::Insert {
                    owner_id: CHAR_ID,
                    item: new_pile.clone(),
                },
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        at(CHAR_ID, ItemLocation::Warehouse).await,
        [(0x6000_1001, 1), (0x6000_1004, 100)]
    );
    assert_eq!(at(CHAR_ID, ItemLocation::Inventory).await, [(0x6000_1003, 400)]);
    // The same ops again (a retry) neither fail nor duplicate.
    store
        .store_item_rows(
            Some(me),
            &[ItemRowOp::Insert {
                owner_id: CHAR_ID,
                item: new_pile.clone(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(at(CHAR_ID, ItemLocation::Warehouse).await.len(), 2);

    // The next full store only knows the bag: the warehouse rows are not "gone".
    data.items = vec![pile_left.clone()];
    store.store_player(&data).await.unwrap();
    assert_eq!(at(CHAR_ID, ItemLocation::Warehouse).await.len(), 2);

    // Withdraw: the sword goes back, 40 of the new pile become a new inventory row.
    let mut back = deposited.clone();
    back.loc = "INVENTORY".into();
    let mut warehouse_pile = new_pile.clone();
    warehouse_pile.count = 60;
    let inventory_pile = item(0x6000_1005, 23, 40, "INVENTORY", 0);
    store
        .store_item_rows(
            Some(me),
            &[
                ItemRowOp::Update {
                    owner_id: CHAR_ID,
                    item: back.clone(),
                },
                ItemRowOp::Update {
                    owner_id: CHAR_ID,
                    item: warehouse_pile.clone(),
                },
                ItemRowOp::Insert {
                    owner_id: CHAR_ID,
                    item: inventory_pile.clone(),
                },
            ],
        )
        .await
        .unwrap();
    assert_eq!(at(CHAR_ID, ItemLocation::Warehouse).await, [(0x6000_1004, 60)]);
    // The store after it has the withdrawn items in the bag and writes no `INSERT` for them.
    data.items = vec![back.clone(), pile_left.clone(), inventory_pile.clone()];
    store.store_player(&data).await.unwrap();
    assert_eq!(
        at(CHAR_ID, ItemLocation::Inventory).await,
        [(0x6000_1001, 1), (0x6000_1003, 400), (0x6000_1005, 40)]
    );

    // The clan warehouse: the owner is the clan, not a character.
    let mut for_clan = back.clone();
    for_clan.loc = "CLANWH".into();
    store
        .store_item_rows(
            Some(me),
            &[ItemRowOp::Write {
                owner_id: CLAN,
                item: for_clan.clone(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(at(CLAN, ItemLocation::ClanWh).await, [(0x6000_1001, 1)]);
    data.items = vec![pile_left.clone(), inventory_pile.clone()];
    store.store_player(&data).await.unwrap();
    assert_eq!(at(CLAN, ItemLocation::ClanWh).await, [(0x6000_1001, 1)]);

    // Freight for another character, then a clan that is dissolved takes its rows with it.
    let mut freight = inventory_pile.clone();
    freight.loc = "FREIGHT".into();
    store
        .store_item_rows(
            Some(me),
            &[ItemRowOp::Write {
                owner_id: MATE,
                item: freight,
            }],
        )
        .await
        .unwrap();
    assert_eq!(at(MATE, ItemLocation::Freight).await, [(0x6000_1005, 40)]);
    store
        .store_item_rows(
            None,
            &[ItemRowOp::Delete {
                object_id: ObjectId(0x6000_1001),
            }],
        )
        .await
        .unwrap();
    assert!(at(CLAN, ItemLocation::ClanWh).await.is_empty());

    pool.close().await;
    db.drop_db().await;
}

#[tokio::test]
async fn augmentations_and_elements_are_written_read_back_and_deleted_with_the_item() {
    use l2_world::effects::item_extras::ItemElemental;

    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    let items = ItemRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    let owner = ObjectId(CHAR_ID);
    let mut store = DbStore::new(pool.clone());
    let sword_id = 0x6000_1001;
    let helmet_id = 0x6000_1002;

    // A new item with an augmentation and two elements: `insertIntoDb` writes the item, then
    // `updateItemAttributes` and `updateItemElements`.
    let mut sword = item(sword_id, 2, 1, "INVENTORY", 0);
    sword.augmentation = Some(0x0003_0002);
    sword.apply_attribute(3, 30);
    sword.apply_attribute(1, 20);
    store
        .store_item_changes(owner, &[sword.clone()], &[])
        .await
        .unwrap();
    assert_eq!(items.augmentation_of(sword_id).await.unwrap(), Some(0x0003_0002));
    let rows = items.elementals_of(sword_id).await.unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| (r.elem_type, r.elem_value))
            .collect::<Vec<_>>(),
        [(1, 20), (3, 30)]
    );

    // The login restore reads them back: the same item, elements in the table's order.
    let mut restored = item(sword_id, 2, 1, "INVENTORY", 0);
    game_server::persist::restore_item_attributes(&items, CHAR_ID, [&mut restored])
        .await
        .unwrap();
    assert_eq!(restored.augmentation, Some(0x0003_0002));
    assert_eq!(
        restored.elementals,
        [
            ItemElemental {
                element: 1,
                value: 20
            },
            ItemElemental {
                element: 3,
                value: 30
            }
        ]
    );

    // A change of the element, and the augmentation taken off (`removeAugmentation`).
    sword.augmentation = None;
    sword.apply_attribute(1, 25);
    store
        .store_item_changes(owner, &[sword.clone()], &[])
        .await
        .unwrap();
    assert_eq!(items.augmentation_of(sword_id).await.unwrap(), None);
    let rows = items.elementals_of(sword_id).await.unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| (r.elem_type, r.elem_value))
            .collect::<Vec<_>>(),
        [(1, 25), (3, 30)]
    );

    // A full store of an item with no attributes leaves another item's rows alone, and an item
    // that is gone takes all its rows with it (`removeFromDb`).
    let mut helmet = item(helmet_id, 6, 1, "INVENTORY", 0);
    helmet.apply_attribute(4, 11);
    store.store_item_changes(owner, &[helmet], &[]).await.unwrap();
    store
        .store_item_changes(owner, &[], &[ObjectId(sword_id)])
        .await
        .unwrap();
    assert!(items.elementals_of(sword_id).await.unwrap().is_empty());
    assert_eq!(items.elementals_of(helmet_id).await.unwrap().len(), 1);

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
    let task = spawn(
        DbStore::new(pool.clone()),
        rx,
        notice_tx,
        RetryPolicy::default(),
        game_server::persist::LeftAcks::default(),
    );
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

/// M5-SHORTCUT-01: the rows of `RequestShortcutReg` and `RequestShortcutDel`, as
/// `Shortcuts.registerShortcutInDb` / `deleteShortcutFromDb` leave the table: one row per
/// (slot, page) with the type ordinal, id and level; a replaced place keeps one row; a deleted
/// place has none; rows of another class index stay.
#[tokio::test]
async fn shortcut_registrations_and_deletions_write_the_rows() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let characters = CharacterRepository::new(pool.clone());
    let shortcuts = ShortcutRepository::new(pool.clone());
    characters.insert_character(&new_character()).await.unwrap();
    let mut store = DbStore::new(pool.clone());
    let sc = |slot, page, kind, shortcut_id, level| ShortcutData {
        slot,
        page,
        kind,
        shortcut_id,
        level,
    };
    let rows = |rows: Vec<l2_db::ShortcutRow>| {
        let mut shape: Vec<(i32, i32, i32, i64, i32)> = rows
            .iter()
            .map(|r| {
                (
                    r.slot,
                    r.page,
                    r.kind.unwrap(),
                    r.shortcut_id.unwrap(),
                    r.level.unwrap(),
                )
            })
            .collect();
        shape.sort_unstable();
        shape
    };
    // A bar of another class index, as the Java server may have written it.
    shortcuts
        .replace_shortcut(
            CHAR_ID,
            1,
            &l2_db::NewShortcut {
                slot: 0,
                page: 0,
                kind: 2,
                shortcut_id: 5,
                level: 1,
            },
        )
        .await
        .unwrap();

    let mut data = player_data();
    data.items = vec![];
    data.skills = vec![];
    // A skill, an item, an action, a macro, a recipe; the last page and slot; a NONE row.
    data.shortcuts = vec![
        sc(0, 0, 2, 1204, 7),
        sc(1, 0, 1, 0x6000_1001, 0),
        sc(2, 0, 3, 5, 0),
        sc(3, 1, 4, 12, 0),
        sc(11, 10, 5, 77, 0),
        sc(4, 0, 0, 0, 0),
    ];
    store.store_player(&data).await.unwrap();
    assert_eq!(
        rows(shortcuts.shortcuts_of(CHAR_ID, 0).await.unwrap()),
        vec![
            (0, 0, 2, 1204, 7),
            (1, 0, 1, 0x6000_1001, 0),
            (2, 0, 3, 5, 0),
            (3, 1, 4, 12, 0),
            (4, 0, 0, 0, 0),
            (11, 10, 5, 77, 0),
        ]
    );

    // A skill level change (`updateShortcuts`), a shortcut that replaces another, one deleted.
    data.shortcuts = vec![
        sc(0, 0, 2, 1204, 8),
        sc(1, 0, 3, 9, 0),
        sc(3, 1, 4, 12, 0),
        sc(11, 10, 5, 77, 0),
        sc(4, 0, 0, 0, 0),
    ];
    store.store_player(&data).await.unwrap();
    assert_eq!(
        rows(shortcuts.shortcuts_of(CHAR_ID, 0).await.unwrap()),
        vec![
            (0, 0, 2, 1204, 8),
            (1, 0, 3, 9, 0),
            (3, 1, 4, 12, 0),
            (4, 0, 0, 0, 0),
            (11, 10, 5, 77, 0),
        ]
    );

    // The other class index is not touched.
    assert_eq!(
        rows(shortcuts.shortcuts_of(CHAR_ID, 1).await.unwrap()),
        vec![(0, 0, 2, 5, 1)]
    );
    pool.close().await;
    db.drop_db().await;
}

// ------------------------------------------------------------------ M5-FU-03: recommendations

/// Java `storeRecommendationValues` is `REPLACE INTO character_reco_bonus (charId,rec_have,rec_left)`:
/// the old row is dropped, so `time_left` falls back to the column default 0.
#[tokio::test]
async fn reco_values_replace_the_row_and_reset_the_time_left() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let reco = RecommendationRepository::new(pool.clone());
    reco.replace_reco_bonus(CHAR_ID, 3, 17, 3_590_000).await.unwrap();
    let mut store = DbStore::new(pool.clone());
    store.store_reco_values(ObjectId(CHAR_ID), 4, 16).await.unwrap();
    let bonus = reco.reco_bonus_of(CHAR_ID).await.unwrap().unwrap();
    assert_eq!((bonus.rec_have, bonus.rec_left, bonus.time_left), (4, 16, 0));

    // A character without a row gets one.
    store
        .store_reco_values(ObjectId(CHAR_ID + 1), 0, 20)
        .await
        .unwrap();
    let bonus = reco.reco_bonus_of(CHAR_ID + 1).await.unwrap().unwrap();
    assert_eq!((bonus.rec_have, bonus.rec_left, bonus.time_left), (0, 20, 0));

    pool.close().await;
    db.drop_db().await;
}

/// `DailyResetManager.resetRecommends`: up to 20 recommendations received go to 0, more lose 20;
/// everybody gets 20 to give and an hour of hourglass.
#[tokio::test]
async fn the_daily_reset_updates_every_reco_row_like_java() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let reco = RecommendationRepository::new(pool.clone());
    // (rec_have before, rec_have after)
    let cases = [(0, 0), (5, 0), (20, 0), (21, 1), (45, 25), (255, 235)];
    for (i, (have, _)) in cases.iter().enumerate() {
        let char_id = CHAR_ID + i32::try_from(i).unwrap();
        reco.replace_reco_bonus(char_id, *have, 3, 12_345).await.unwrap();
    }
    let mut store = DbStore::new(pool.clone());
    store.reset_recommends().await.unwrap();
    for (i, (_, after)) in cases.iter().enumerate() {
        let char_id = CHAR_ID + i32::try_from(i).unwrap();
        let bonus = reco.reco_bonus_of(char_id).await.unwrap().unwrap();
        assert_eq!(
            (bonus.rec_have, bonus.rec_left, bonus.time_left),
            (*after, 20, 3_600_000),
            "row {i}"
        );
    }

    pool.close().await;
    db.drop_db().await;
}

/// `PlayerVariables.storeMe` through `store_player_variables`: deletes, then upserts.
#[tokio::test]
async fn player_variables_are_upserted_and_deleted() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let variables = VariableRepository::new(pool.clone());
    variables
        .upsert_variable(CHAR_ID, "nevit_b", "100")
        .await
        .unwrap();
    variables
        .upsert_variable(CHAR_ID, "hunting_time", "500")
        .await
        .unwrap();
    variables
        .upsert_variable(CHAR_ID, "UI_KEY_MAPPING", "1")
        .await
        .unwrap();
    let mut store = DbStore::new(pool.clone());
    store
        .store_player_variables(
            ObjectId(CHAR_ID),
            &[("hunting_time".to_owned(), "0".to_owned())],
            &["nevit_b".to_owned()],
        )
        .await
        .unwrap();
    let mut stored = variables.variables_of(CHAR_ID).await.unwrap();
    stored.sort();
    assert_eq!(
        stored,
        vec![
            ("UI_KEY_MAPPING".to_owned(), "1".to_owned()),
            ("hunting_time".to_owned(), "0".to_owned()),
        ]
    );

    pool.close().await;
    db.drop_db().await;
}

/// M6-SYNC-01: the reads that keep the mail and the clan warehouse of this server in step with the
/// Java game server, which shares the rows (D-39). Only reads; the rows come from the writes of
/// the other server, here made with the store.
#[tokio::test]
async fn the_mail_and_clan_warehouse_reads_see_what_the_database_has_now() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    const SENDER: i32 = CHAR_ID + 1;
    const MAIL_A: i32 = 0x1000_0101;
    const MAIL_B: i32 = 0x1000_0102;
    const CLAN: i32 = 500;
    let messages = l2_db::MessageRepository::new(pool.clone());
    let items = ItemRepository::new(pool.clone());
    let mut store = DbStore::new(pool.clone());
    let mail = |id: i32, sender: i32, receiver: i32, has_attachments: bool| l2_db::MessageRow {
        message_id: id,
        sender_id: sender,
        receiver_id: receiver,
        subject: "s".into(),
        content: "t".into(),
        expiration: NOW,
        req_adena: 0,
        has_attachments,
        is_unread: true,
        is_deleted_by_sender: false,
        is_deleted_by_receiver: false,
        send_by_system: 0,
        is_returned: false,
    };
    messages
        .insert(&mail(MAIL_A, SENDER, CHAR_ID, true))
        .await
        .unwrap();
    messages
        .insert(&mail(MAIL_B, SENDER, CHAR_ID + 7, false))
        .await
        .unwrap();
    store
        .store_item_rows(
            None,
            &[
                ItemRowOp::Insert {
                    owner_id: SENDER,
                    item: item(0x1000_0201, 57, 3, "MAIL", MAIL_A),
                },
                ItemRowOp::Insert {
                    owner_id: SENDER,
                    item: item(0x1000_0202, 57, 5, "MAIL", MAIL_A),
                },
                // An attachment of another mail of the same sender.
                ItemRowOp::Insert {
                    owner_id: SENDER,
                    item: item(0x1000_0203, 57, 9, "MAIL", MAIL_B),
                },
                ItemRowOp::Insert {
                    owner_id: CLAN,
                    item: item(0x1000_0301, 57, 50, "CLANWH", 0),
                },
                ItemRowOp::Insert {
                    owner_id: CLAN,
                    item: item(0x1000_0302, 57, 60, "WAREHOUSE", 0),
                },
            ],
        )
        .await
        .unwrap();

    // The character select: both mails of the sender are not the character's, one is.
    let refresh = game_server::persist::load_mail_of_character(&messages, &items, CHAR_ID)
        .await
        .unwrap();
    assert_eq!(refresh.messages.len(), 1);
    let loaded = &refresh.messages[0];
    assert_eq!(
        (loaded.id, loaded.sender_id, loaded.receiver_id),
        (MAIL_A, SENDER, CHAR_ID)
    );
    let mut attached: Vec<(i32, i64)> = loaded
        .attachments
        .iter()
        .map(|i| (i.object_id.0, i.count))
        .collect();
    attached.sort_unstable();
    assert_eq!(attached, [(0x1000_0201, 3), (0x1000_0202, 5)]);
    // The sender sees both mails, each with its own items.
    let sent = game_server::persist::load_mail_of_character(&messages, &items, SENDER)
        .await
        .unwrap();
    assert_eq!(sent.messages.len(), 2);

    // The attachments of one message: only the rows still `MAIL` with that `loc_data`.
    let rows = store.load_mail_attachments(SENDER, MAIL_A).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.loc == "MAIL" && r.loc_data == MAIL_A));
    // The Java game server gave one out: its row moved to a character's inventory.
    items
        .update_item(
            0x1000_0201,
            &l2_db::ItemUpdate {
                owner_id: CHAR_ID,
                count: 3,
                loc: ItemLocation::Inventory,
                loc_data: 0,
                enchant_level: 0,
                custom_type1: 0,
                custom_type2: 0,
                mana_left: -1,
                time: -1,
            },
        )
        .await
        .unwrap();
    let rows = store.load_mail_attachments(SENDER, MAIL_A).await.unwrap();
    assert_eq!(
        rows.iter().map(|r| r.object_id.0).collect::<Vec<_>>(),
        [0x1000_0202]
    );
    assert!(
        store
            .load_mail_attachments(SENDER, 12345)
            .await
            .unwrap()
            .is_empty()
    );

    // The clan warehouse: the CLANWH rows of the clan only.
    let wh = store.load_clan_warehouse(CLAN).await.unwrap();
    assert_eq!(
        wh.iter().map(|r| r.object_id.0).collect::<Vec<_>>(),
        [0x1000_0301]
    );
    assert!(store.load_clan_warehouse(501).await.unwrap().is_empty());

    db.drop_db().await;
}
