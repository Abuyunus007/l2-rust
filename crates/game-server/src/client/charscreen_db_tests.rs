//! The character screen against a throwaway MariaDB: the rows of a new character, the delete
//! timer, restore, the cascade of a real delete and the online flag.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service). The
//! test databases are named `l2_test_charscreen_*` and dropped at the end; never point the URL at
//! a server with real data.

use std::sync::Arc;

use l2_config::GameConfig;
use l2_db::{
    CharacterRepository, ItemLocation, ItemRepository, Pool, ShortcutRepository, SkillRepository,
    connect_with,
};
use l2_protocol_game::client::CharacterCreate;
use l2_testkit::test_db::TestDb;
use l2_util::WorldRng;
use l2_util::time::now_millis;
use l2_world::IdFactory;
use sqlx::mysql::MySqlConnectOptions;

use super::charlist::{CharacterSource, DbCharacterList};
use super::charscreen::{CharScreen, CharScreenStore, DbCharScreen, DeleteOutcome, create_character};
use super::testdata::datapack;

/// Every game table the character screen writes or deletes from.
const SCHEMA: [&str; 25] = [
    include_str!("../../../l2-db/sql/game/characters.sql"),
    include_str!("../../../l2-db/sql/game/items.sql"),
    include_str!("../../../l2-db/sql/game/item_attributes.sql"),
    include_str!("../../../l2-db/sql/game/character_skills.sql"),
    include_str!("../../../l2-db/sql/game/character_skills_save.sql"),
    include_str!("../../../l2-db/sql/game/character_item_reuse_save.sql"),
    include_str!("../../../l2-db/sql/game/character_shortcuts.sql"),
    include_str!("../../../l2-db/sql/game/character_macroses.sql"),
    include_str!("../../../l2-db/sql/game/character_reco_bonus.sql"),
    include_str!("../../../l2-db/sql/game/character_contacts.sql"),
    include_str!("../../../l2-db/sql/game/character_friends.sql"),
    include_str!("../../../l2-db/sql/game/character_hennas.sql"),
    include_str!("../../../l2-db/sql/game/character_quests.sql"),
    include_str!("../../../l2-db/sql/game/character_recipebook.sql"),
    include_str!("../../../l2-db/sql/game/character_subclasses.sql"),
    include_str!("../../../l2-db/sql/game/heroes.sql"),
    include_str!("../../../l2-db/sql/game/olympiad_nobles.sql"),
    include_str!("../../../l2-db/sql/game/seven_signs.sql"),
    include_str!("../../../l2-db/sql/game/pets.sql"),
    include_str!("../../../l2-db/sql/game/merchant_lease.sql"),
    include_str!("../../../l2-db/sql/game/character_raid_points.sql"),
    include_str!("../../../l2-db/sql/game/character_instance_time.sql"),
    include_str!("../../../l2-db/sql/game/character_variables.sql"),
    include_str!("../../../l2-db/sql/game/mods_wedding.sql"),
    include_str!("../../../l2-db/sql/game/clan_data.sql"),
];

const ACCOUNT: &str = "acc";

async fn setup() -> Option<(TestDb, Pool)> {
    let db = TestDb::create("charscreen").await?;
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

fn packet(name: &str, class_id: i32, female: bool) -> CharacterCreate {
    CharacterCreate {
        name: name.to_owned(),
        race: 0,
        is_female: female,
        class_id,
        int: 0,
        str: 0,
        con: 0,
        men: 0,
        dex: 0,
        wit: 0,
        hair_style: 2,
        hair_color: 1,
        face: 1,
    }
}

async fn count(pool: &Pool, table: &str, column: &str, id: i32) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM {table} WHERE {column} = {id}"
    )))
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn a_created_character_has_the_rows_java_writes() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let (_dir, pack) = datapack();
    let ids = Arc::new(IdFactory::new([]).unwrap());
    let mut config = GameConfig::default();
    config.manual.player.forbidden_names = vec!["annou".into()];
    let screen = CharScreen::new(
        Arc::new(DbCharScreen::new(pool.clone())),
        pack,
        Arc::new(config),
        Arc::clone(&ids),
        0,
    );
    let mut rng = WorldRng::from_seed(7);
    let plan = create_character(&screen, ACCOUNT, &packet("Mystic", 10, true), &mut rng)
        .await
        .unwrap();
    let id = plan.char_id;
    assert!(id >= 0x6000_0000);

    // characters: INSERT, then storeMe's UPDATE, then deleteMe's online/lastAccess.
    let row = CharacterRepository::new(pool.clone())
        .restore_character(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.account_name.as_deref(), Some(ACCOUNT));
    assert_eq!(row.char_name, "Mystic");
    assert_eq!((row.level, row.exp, row.sp), (Some(1), Some(0), 0));
    assert_eq!((row.max_hp, row.cur_hp), (Some(95), Some(95)));
    assert_eq!((row.max_mp, row.cur_mp), (Some(100), Some(58)));
    assert_eq!((row.max_cp, row.cur_cp), (Some(53 * 9 / 10), Some(0)));
    assert_eq!(
        (row.sex, row.face, row.hair_style, row.hair_color),
        (Some(1), Some(1), Some(2), Some(1))
    );
    assert_eq!((row.race, row.classid, row.base_class), (Some(0), Some(10), 10));
    assert_eq!(row.title.as_deref(), Some(""));
    assert_eq!(row.title_color, 0xECF9A2);
    assert_eq!((row.online, row.deletetime, row.cancraft), (Some(0), 0, Some(0)));
    assert_eq!((row.newbie, row.nobless, row.accesslevel), (Some(1), 0, Some(0)));
    assert_eq!((row.x, row.y, row.z), (Some(28295), Some(11063), Some(-4224)));
    assert_eq!((row.vitality_points, row.clanid), (20000, Some(0)));
    assert!(row.language.is_none());
    assert_eq!(row.last_access, u64::try_from(plan.last_access).unwrap());

    // items: owner, location, slot, the Rust id range.
    let items = ItemRepository::new(pool.clone())
        .inventory_items(id, ItemLocation::Inventory, ItemLocation::Paperdoll)
        .await
        .unwrap();
    assert_eq!(items.len(), 5);
    assert!(items.iter().all(|i| i.object_id >= 0x6000_0000));
    let mut summary: Vec<_> = items
        .iter()
        .map(|i| {
            (
                i.item_id.unwrap(),
                i.count,
                i.loc.clone().unwrap(),
                i.loc_data.unwrap(),
            )
        })
        .collect();
    summary.sort();
    assert_eq!(
        summary,
        [
            (6, 1, "PAPERDOLL".to_owned(), 5),
            (425, 1, "PAPERDOLL".to_owned(), 6),
            (461, 1, "PAPERDOLL".to_owned(), 11),
            (5588, 1, "INVENTORY".to_owned(), 0),
            (10650, 5, "INVENTORY".to_owned(), 0),
        ]
    );
    assert!(items.iter().all(|i| i.mana_left == -1 && i.time == -1));

    // skills and shortcuts of class index 0.
    let skills = SkillRepository::new(pool.clone()).skills_of(id, 0).await.unwrap();
    let mut skills: Vec<_> = skills.iter().map(|s| (s.skill_id, s.skill_level)).collect();
    skills.sort();
    assert_eq!(skills, [(194, 1), (1177, 1), (1216, 1), (1322, 1)]);
    let guide = items.iter().find(|i| i.item_id == Some(5588)).unwrap().object_id;
    let mut bar: Vec<_> = ShortcutRepository::new(pool.clone())
        .shortcuts_of(id, 0)
        .await
        .unwrap()
        .iter()
        .map(|s| (s.slot, s.page, s.kind, s.shortcut_id, s.level))
        .collect();
    bar.sort();
    assert_eq!(
        bar,
        [
            (0, 0, Some(3), Some(2), Some(0)),
            (1, 0, Some(2), Some(1177), Some(1)),
            (3, 0, Some(3), Some(5), Some(0)),
            (9, 0, Some(2), Some(1216), Some(1)),
            (10, 0, Some(3), Some(0), Some(0)),
            (11, 0, Some(1), Some(i64::from(guide)), Some(0)),
        ]
    );

    // character_reco_bonus: deleteMe's storeRecommendations.
    let reco: (i64, i64, i64) = sqlx::query_as(
        "SELECT CAST(rec_have AS SIGNED), CAST(rec_left AS SIGNED), CAST(time_left AS SIGNED) FROM character_reco_bonus WHERE charId = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reco, (0, 20, 0));

    // Nothing else is written for a new character.
    for (table, column) in [
        ("character_variables", "charId"),
        ("character_quests", "charId"),
        ("character_hennas", "charId"),
        ("character_macroses", "charId"),
        ("character_skills_save", "charId"),
        ("character_subclasses", "charId"),
        ("character_recipebook", "charId"),
        ("character_contacts", "charId"),
        ("character_friends", "charId"),
        ("seven_signs", "charId"),
        ("heroes", "charId"),
    ] {
        assert_eq!(count(&pool, table, column, id).await, 0, "{table}");
    }
    assert_eq!(count(&pool, "characters", "charId", id).await, 1);

    // The checks the packet handler runs against the table.
    assert_eq!(screen.store.account_character_count(ACCOUNT).await, 1);
    assert_eq!(screen.store.account_character_count("other").await, 0);
    assert!(screen.store.name_exists("Mystic").await);
    assert!(
        screen.store.name_exists("mystic").await,
        "the column is case insensitive"
    );
    assert!(!screen.store.name_exists("Nobody").await);

    pool.close().await;
    db.drop_db().await;
}

#[tokio::test]
async fn delete_timer_restore_clan_rules_and_cascade() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let (_dir, pack) = datapack();
    let ids = Arc::new(IdFactory::new([]).unwrap());
    let mut config = GameConfig::default();
    config.manual.player.forbidden_names = vec!["annou".into()];
    let store = Arc::new(DbCharScreen::new(pool.clone()));
    let screen = CharScreen::new(store.clone(), pack, Arc::new(config), ids, 0);
    let mut rng = WorldRng::from_seed(7);
    let plan = create_character(&screen, ACCOUNT, &packet("Fighter", 0, false), &mut rng)
        .await
        .unwrap();
    let id = plan.char_id;
    let characters = CharacterRepository::new(pool.clone());
    let deletetime = || async {
        characters
            .restore_character(id)
            .await
            .unwrap()
            .unwrap()
            .deletetime
    };

    // 7 days: only the timer is set.
    let now = now_millis();
    assert_eq!(
        store.mark_to_delete(id, 7, false, now).await.unwrap(),
        DeleteOutcome::Done
    );
    assert_eq!(i64::try_from(deletetime().await).unwrap(), now + 7 * 86_400_000);
    assert_eq!(count(&pool, "items", "owner_id", id).await, 6);

    // Restore takes it back.
    store.restore(id).await.unwrap();
    assert_eq!(deletetime().await, 0);

    // A clan member and a clan leader may not be deleted; a missing clan does not count.
    let other = 0x6fff_0000;
    db.exec(&format!(
        "INSERT INTO clan_data (clan_id, clan_name, leader_id) VALUES (900, 'Clan', {other})"
    ))
    .await;
    db.exec(&format!("UPDATE characters SET clanid = 900 WHERE charId = {id}"))
        .await;
    assert_eq!(
        store.mark_to_delete(id, 7, false, now).await.unwrap(),
        DeleteOutcome::ClanMember
    );
    db.exec(&format!(
        "UPDATE clan_data SET leader_id = {id} WHERE clan_id = 900"
    ))
    .await;
    assert_eq!(
        store.mark_to_delete(id, 7, false, now).await.unwrap(),
        DeleteOutcome::ClanLeader
    );
    assert_eq!(deletetime().await, 0, "a refused delete leaves the timer alone");
    db.exec("DELETE FROM clan_data WHERE clan_id = 900").await;
    assert_eq!(
        store.mark_to_delete(id, 7, false, now).await.unwrap(),
        DeleteOutcome::Done
    );
    store.restore(id).await.unwrap();

    // Online flag.
    store.set_online(id, 1, 123_456).await.unwrap();
    let row = characters.restore_character(id).await.unwrap().unwrap();
    assert_eq!((row.online, row.last_access), (Some(1), 123_456));

    // DeleteCharAfterDays = 0: gone at once, with everything that belonged to it.
    db.exec(&format!(
        "INSERT INTO character_variables (charId, var, val) VALUES ({id}, 'x', 'y')"
    ))
    .await;
    db.exec(&format!(
        "INSERT INTO character_friends (charId, friendId) VALUES ({other}, {id})"
    ))
    .await;
    assert_eq!(
        store.mark_to_delete(id, 0, false, now).await.unwrap(),
        DeleteOutcome::Done
    );
    for (table, column) in [
        ("characters", "charId"),
        ("items", "owner_id"),
        ("character_skills", "charId"),
        ("character_shortcuts", "charId"),
        ("character_reco_bonus", "charId"),
        ("character_variables", "charId"),
        ("character_friends", "friendId"),
    ] {
        assert_eq!(count(&pool, table, column, id).await, 0, "{table}");
    }

    pool.close().await;
    db.drop_db().await;
}

#[tokio::test]
async fn the_character_list_deletes_characters_whose_timer_ran_out() {
    let Some((db, pool)) = setup().await else {
        return;
    };
    let (_dir, pack) = datapack();
    let ids = Arc::new(IdFactory::new([]).unwrap());
    let mut config = GameConfig::default();
    config.manual.player.forbidden_names = vec!["annou".into()];
    let screen = CharScreen::new(
        Arc::new(DbCharScreen::new(pool.clone())),
        Arc::clone(&pack),
        Arc::new(config),
        ids,
        0,
    );
    let mut rng = WorldRng::from_seed(7);
    let gone = create_character(&screen, ACCOUNT, &packet("Gone", 0, false), &mut rng)
        .await
        .unwrap()
        .char_id;
    let waiting = create_character(&screen, ACCOUNT, &packet("Waiting", 0, false), &mut rng)
        .await
        .unwrap()
        .char_id;
    let now = now_millis();
    db.exec(&format!(
        "UPDATE characters SET deletetime = {} WHERE charId = {gone}",
        now - 1000
    ))
    .await;
    db.exec(&format!(
        "UPDATE characters SET deletetime = {} WHERE charId = {waiting}",
        now + 600_000
    ))
    .await;

    let list = DbCharacterList::new(pool.clone(), pack, false)
        .character_list(ACCOUNT)
        .await;
    let names: Vec<_> = list.iter().map(|c| c.entry.name.as_str()).collect();
    assert_eq!(names, ["Waiting"]);
    assert!(list[0].entry.delete_seconds > 590);
    assert_eq!(count(&pool, "characters", "charId", gone).await, 0);
    assert_eq!(count(&pool, "items", "owner_id", gone).await, 0);
    assert_eq!(count(&pool, "characters", "charId", waiting).await, 1);

    pool.close().await;
    db.drop_db().await;
}
