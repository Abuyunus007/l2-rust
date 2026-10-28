#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Entering the world, chat, logout and restart, driven synchronously with recording sinks.
//! The comparison with the recorded Java session is in `enter_capture.rs`.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::enterworld::{
    EtcStatusUpdate, ExStorageMaxCount, ExUiSetting, ExVoteSystemInfo, HennaInfo, ShortcutEntry,
    ShortcutInit, SkillCoolTime, SkillList,
};
use l2_protocol_game::server::{
    ActionFailed, ChatType, CreatureSay, DeleteObject, ExQuestItemList, ItemList, LeaveWorld, MacroList,
    NetPing, RestartResponse, SayBody, SenderField, ServerClose, SystemMessage, UserInfo,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::*;

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(i32, Vec<u8>),
    Left(i32, LeaveReason),
    Persist(PersistCommand),
    Ls(LsEvent),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }

    /// The packets sent to player `id`, in order.
    fn packets(&self, id: i32) -> Vec<Vec<u8>> {
        self.take()
            .into_iter()
            .filter_map(|o| match o {
                Out::Packet(to, bytes) if to == id => Some(bytes),
                _ => None,
            })
            .collect()
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push(Out::Packet(self.0, bytes));
    }
    fn left_world(&self, reason: LeaveReason) {
        self.1.0.lock().unwrap().push(Out::Left(self.0, reason));
    }
}

struct RecOutbox(Log);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, event: LsEvent) {
        self.0.0.lock().unwrap().push(Out::Ls(event));
    }
}

/// A datapack view with a few items, one GM level and the class template of the test character.
#[derive(Default)]
struct FakeData {
    items: HashMap<i32, ItemStatic>,
    passive: HashSet<i32>,
    stats: StatData,
}

/// Class 10 (the test character) with 100 HP, 50 MP and 10 CP at level 30, the values the
/// character table stores for it. No `statBonus.xml`: every bonus is 1.
fn class_templates() -> l2_data::PlayerTemplates {
    let mut levels = String::new();
    for level in 1..=30 {
        let (hp, mp, cp) = if level == 30 {
            (100, 50, 10)
        } else {
            (level, level, level)
        };
        levels.push_str(&format!(
            "<level val=\"{level}\"><hp>{hp}</hp><mp>{mp}</mp><cp>{cp}</cp></level>"
        ));
    }
    let xml = format!(
        r#"<list><classId>10</classId><staticData>
<baseINT>41</baseINT><baseSTR>22</baseSTR><baseCON>27</baseCON><baseMEN>39</baseMEN><baseDEX>21</baseDEX><baseWIT>20</baseWIT>
<collisionMale><radius>7.5</radius><height>22.8</height></collisionMale>
<collisionFemale><radius>6.5</radius><height>22.5</height></collisionFemale>
</staticData><lvlUpgainData>{levels}</lvlUpgainData></list>"#
    );
    l2_data::PlayerTemplates::from_xml(&xml, 85).unwrap()
}

impl StaticData for FakeData {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        self.items.get(&item_id).cloned()
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.passive.contains(&skill_id)
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.25
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        AccessLevelStatic {
            is_gm: level >= 100,
            ..AccessLevelStatic::USER
        }
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

fn item_static(display_id: i32, type2: i16, quest: bool, weight: i32) -> ItemStatic {
    ItemStatic {
        display_id,
        type2,
        body_part: 0x0080,
        quest_item: quest,
        weight,
        shared_reuse_group: -1,
        weapon: type2 == 0,
    }
}

fn item(object_id: i32, item_id: i32, count: i64, loc: &str, loc_data: i32) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
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

fn player(id: i32, account: &str, name: &str) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: account.into(),
        name: name.into(),
        title: "Title".into(),
        class_id: 10,
        base_class: 10,
        race: 0,
        sex: 1,
        face: 1,
        hair_style: 2,
        hair_color: 3,
        level: 30,
        exp: 1000,
        sp: 77,
        karma: 0,
        pvp_kills: 1,
        pk_kills: 2,
        access_level: 0,
        x: -71_338,
        y: 258_271,
        z: -3_104,
        heading: 1234,
        cur_hp: 90.4,
        max_hp: 100.0,
        cur_mp: 49.5,
        max_mp: 50.0,
        cur_cp: 7.9,
        max_cp: 10.0,
        online_time: 10,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn data() -> FakeData {
    let mut data = FakeData {
        stats: StatData {
            player_templates: Arc::new(class_templates()),
            ..StatData::default()
        },
        ..FakeData::default()
    };
    data.items.insert(6, item_static(6, 0, false, 10));
    data.items.insert(57, item_static(57, 4, false, 0));
    data.items.insert(1235, item_static(1235, 3, true, 1));
    data.items.insert(736, item_static(736, 5, false, 5));
    data.passive.insert(141);
    data
}

fn world(log: &Log) -> World {
    let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
    world.set_static_data(Box::new(data()));
    world
}

fn enter_player(world: &mut World, log: &Log, session: u64, data: PlayerData) {
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(session),
        player: Box::new(data),
        sink: Box::new(Sink(data_id(session), log.clone())),
        ip: None,
    });
}

/// The sink of a session is numbered by its object id; tests use `session == id`.
fn data_id(session: u64) -> i32 {
    i32::try_from(session).unwrap()
}

fn say(world: &mut World, session: u64, chat_type: i32, text: &str, target: Option<&str>) {
    world.handle(WorldCommand::Input {
        session: SessionId(session),
        input: ClientInput::Say {
            chat_type,
            text: text.into(),
            target: target.map(str::to_owned),
        },
    });
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn names(packets: &[Vec<u8>]) -> Vec<String> {
    packets.iter().map(|p| name_of(p)).collect()
}

fn decode<P: Decode>(packet: &[u8]) -> P {
    let skip = if packet[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&packet[skip..])).unwrap()
}

fn first<'a>(packets: &'a [Vec<u8>], name: &str) -> &'a Vec<u8> {
    packets.iter().find(|p| name_of(p) == name).unwrap()
}

fn text(message: &str) -> Vec<u8> {
    SystemMessage::text(message).to_bytes()
}

/// A player with one of everything: a wand in the right hand, adena, a quest item, a plain item.
fn equipped(id: i32, name: &str) -> PlayerData {
    let mut p = player(id, "acc", name);
    p.items = vec![
        item(0x2000_0001, 6, 1, "PAPERDOLL", 5),
        item(0x2000_0002, 57, 5_000, "INVENTORY", 0),
        item(0x2000_0003, 1235, 3, "INVENTORY", 1),
        item(0x2000_0004, 736, 2, "INVENTORY", 2),
        // Not part of the inventory: stays out of every list.
        item(0x2000_0005, 57, 9, "WAREHOUSE", 0),
    ];
    p.skills = vec![
        SkillData {
            skill_id: 141,
            skill_level: 2,
        },
        SkillData {
            skill_id: 1177,
            skill_level: 5,
        },
    ];
    p
}

// ---- the enter-world sequence -----------------------------------------------------------------

#[test]
fn entering_sends_the_enter_world_packets_in_java_order() {
    let log = Log::default();
    let mut w = world(&log);
    enter_player(&mut w, &log, 1, equipped(1, "Hero"));
    let out = log.take();
    // The world accepted the player before it said anything, and told the login server.
    assert!(
        matches!(out.first(), Some(Out::Ls(LsEvent::PlayerInGame { .. }))),
        "{out:?}"
    );
    let packets: Vec<Vec<u8>> = out
        .into_iter()
        .filter_map(|o| match o {
            Out::Packet(1, bytes) => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(
        names(&packets),
        [
            "USER_INFO",
            "EX_VOTE_SYSTEM_INFO",
            "MACRO_LIST",
            "ITEM_LIST",
            "EX_QUEST_ITEM_LIST",
            "EX_GET_BOOKMARK_INFO",
            "SHORT_CUT_INIT",
            "EX_BASIC_ACTION_LIST",
            "HENNA_INFO",
            "QUEST_LIST",
            "EX_ROTATION",
            "ETC_STATUS_UPDATE",
            "EX_STORAGE_MAX_COUNT",
            "L2_FRIEND_LIST",
            "SYSTEM_MESSAGE",
            // `SevenSigns.sendCurrentPeriodMsg` (M8-SS-01).
            "SYSTEM_MESSAGE",
            "SKILL_COOL_TIME",
            "EX_VOTE_SYSTEM_INFO",
            "EX_RECEIVE_SHOW_POST_FRIEND",
            "EX_NAVIT_ADVENT_POINT_INFO",
            "EX_NAVIT_ADVENT_TIME_CHANGE",
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "VALIDATE_LOCATION",
            "ACTION_FAIL",
            "SKILL_LIST",
        ]
    );
    assert_eq!(
        packets[14],
        SystemMessage::new(SystemMessageId::WelcomeToTheWorldOfLineageIi).to_bytes()
    );
}

#[test]
fn nevit_off_leaves_out_its_packets() {
    let log = Log::default();
    let mut w = world(&log);
    w.set_settings(WorldSettings {
        nevit_enabled: false,
        ..WorldSettings::default()
    });
    enter_player(&mut w, &log, 1, equipped(1, "Hero"));
    let list = names(&log.packets(1));
    assert!(
        !list.iter().any(|n| n.contains("VOTE") || n.contains("NAVIT")),
        "{list:?}"
    );
    assert_eq!(list.len(), 22);
}

#[test]
fn user_info_describes_the_character_and_the_second_one_knows_the_recommendations() {
    let log = Log::default();
    let mut w = world(&log);
    let mut p = equipped(1, "Hero");
    p.profile.recom_left = 20;
    p.profile.recom_have = 3;
    p.profile.fame = 12;
    p.profile.noble = true;
    p.profile.vitality_points = 20_000;
    enter_player(&mut w, &log, 1, p);
    let packets = log.packets(1);
    let infos: Vec<&Vec<u8>> = packets.iter().filter(|p| name_of(p) == "USER_INFO").collect();
    assert_eq!(infos.len(), 2);
    let (a, b): (UserInfo, UserInfo) = (decode(infos[0]), decode(infos[1]));

    assert_eq!((a.name.as_str(), a.object_id), ("Hero", 1));
    assert_eq!((a.x, a.y, a.z), (-71_338, 258_271, -3_104));
    assert_eq!((a.level, a.exp, a.sp), (30, 1000, 77));
    assert_eq!(a.exp_percent, 0.25);
    assert_eq!((a.max_hp, a.current_hp), (100, 90));
    // Java rounds the current values: 49.5 -> 50
    assert_eq!((a.max_mp, a.current_mp), (50, 50));
    // `(int) getCurrentCp()` truncates.
    assert_eq!((a.max_cp, a.current_cp), (10, 7));
    assert!(a.is_female);
    assert_eq!(a.class_id, 10);
    assert_eq!((a.pk_kills, a.pvp_kills), (2, 1));
    assert!(a.noble);
    assert_eq!(a.pledge_class, 5);
    assert_eq!((a.fame, a.vitality_points), (12, 20_000));
    // The wand is in the right hand; the packet lists object id and display id per slot.
    assert_eq!(a.paperdoll_object_ids[paperdoll::RHAND], 0x2000_0001);
    assert_eq!(a.paperdoll_item_ids[paperdoll::RHAND], 6);
    assert_eq!(a.paperdoll_object_ids[paperdoll::CHEST], 0);
    // 10 (wand) + 5 (item 736 x2 = 10) + 1 (quest item x3 = 3), warehouse ignored.
    assert_eq!(a.current_load, 10 + 3 + 10);
    // Java `getMaxLoad()`: floor(CON bonus * 69000 * AltWeightLimit), the bonus is 1 here.
    assert_eq!(a.max_load, 69_000);
    assert_eq!(a.inventory_limit, 80);
    // Java: the first UserInfo is built before the recommendations are loaded, and before the
    // player has "entered": it still lets the client equip a cloak.
    assert_eq!((a.recom_left, a.recom_have), (0, 0));
    assert!(a.can_equip_cloak);
    assert_eq!((b.recom_left, b.recom_have), (20, 3));
    assert!(!b.can_equip_cloak);
    // Nothing else differs.
    let mut b_like_a = b.clone();
    b_like_a.recom_left = 0;
    b_like_a.recom_have = 0;
    b_like_a.can_equip_cloak = true;
    assert_eq!(b_like_a, a);
    // The world can build the same packet on demand.
    assert_eq!(w.user_info(ObjectId(1)).unwrap(), b);
}

#[test]
fn item_list_and_quest_item_list_split_the_inventory() {
    let log = Log::default();
    let mut w = world(&log);
    enter_player(&mut w, &log, 1, equipped(1, "Hero"));
    let packets = log.packets(1);
    let list: ItemList = decode(first(&packets, "ITEM_LIST"));
    assert!(!list.show_window);
    let ids: Vec<(i32, i32, i16, i16)> = list
        .items
        .iter()
        .map(|i| (i.object_id, i.item_id, i.type2, i.equipped))
        .collect();
    assert_eq!(
        ids,
        [
            (0x2000_0001, 6, 0, 1),
            (0x2000_0002, 57, 4, 0),
            (0x2000_0004, 736, 5, 0),
        ]
    );
    assert_eq!(list.items[0].location, 5);
    assert_eq!(list.items[1].count, 5_000);
    assert_eq!((list.items[0].mana, list.items[0].time), (-1, -9999));
    let quest: ExQuestItemList = decode(first(&packets, "EX_QUEST_ITEM_LIST"));
    assert_eq!(quest.items.len(), 1);
    assert_eq!((quest.items[0].object_id, quest.items[0].count), (0x2000_0003, 3));
}

#[test]
fn shadow_and_time_limited_items_report_mana_and_time() {
    let log = Log::default();
    let mut w = world(&log);
    let mut p = player(1, "acc", "Hero");
    let mut shadow = item(0x2000_0001, 6, 1, "INVENTORY", 0);
    shadow.mana_left = 540;
    shadow.enchant_level = 4;
    let mut timed = item(0x2000_0002, 736, 1, "INVENTORY", 1);
    // 90 seconds from now (the item's `time` is the end in unix milliseconds)
    timed.time = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
        + 90_500;
    p.items = vec![shadow, timed];
    enter_player(&mut w, &log, 1, p);
    let packets = log.packets(1);
    let list: ItemList = decode(first(&packets, "ITEM_LIST"));
    assert_eq!((list.items[0].mana, list.items[0].enchant), (540, 4));
    assert!((88..=90).contains(&list.items[1].time), "{}", list.items[1].time);
}

#[test]
fn skill_list_marks_passive_skills() {
    let log = Log::default();
    let mut w = world(&log);
    enter_player(&mut w, &log, 1, equipped(1, "Hero"));
    let packets = log.packets(1);
    let list: SkillList = decode(packets.last().unwrap());
    let skills: Vec<_> = list.skills.iter().map(|s| (s.id, s.level, s.passive)).collect();
    assert_eq!(skills, [(141, 2, true), (1177, 5, false)]);
    // The session sends the same list when the character is selected.
    assert_eq!(
        packets.last().unwrap(),
        &l2_world::enter::skill_list(&equipped(1, "Hero"), &data()).to_bytes()
    );
}

#[test]
fn shortcuts_follow_the_java_wire_form_and_drop_dead_item_links() {
    let log = Log::default();
    let mut w = world(&log);
    let mut p = equipped(1, "Hero");
    let sc = |slot, page, kind, shortcut_id, level| ShortcutData {
        slot,
        page,
        kind,
        shortcut_id,
        level,
    };
    p.shortcuts = vec![
        sc(0, 0, 2, 1177, 5),        // skill
        sc(1, 0, 1, 0x2000_0004, 0), // item that exists
        sc(2, 0, 1, 0x7FFF_0000, 0), // item that is gone: left out
        sc(3, 1, 3, 2, 0),           // action on page 1
        sc(4, 2, 4, 1001, 0),        // macro on page 2
        sc(5, 0, 5, 77, 0),          // recipe
        sc(6, 0, 6, 8, 0),           // bookmark
        sc(7, 0, 99, 1, 0),          // not a shortcut type
        sc(8, 0, 0, 0, 0),           // NONE
    ];
    enter_player(&mut w, &log, 1, p);
    let packets = log.packets(1);
    let init: ShortcutInit = decode(first(&packets, "SHORT_CUT_INIT"));
    assert_eq!(
        init.shortcuts,
        [
            ShortcutEntry::Skill {
                slot: 0,
                id: 1177,
                level: 5
            },
            ShortcutEntry::Item {
                slot: 1,
                id: 0x2000_0004,
                shared_reuse_group: -1
            },
            // Java writes `slot + page * 12`.
            ShortcutEntry::Other {
                kind: l2_protocol_game::server::enterworld::OtherShortcut::Action,
                slot: 15,
                id: 2
            },
            ShortcutEntry::Other {
                kind: l2_protocol_game::server::enterworld::OtherShortcut::Macro,
                slot: 28,
                id: 1001
            },
            ShortcutEntry::Other {
                kind: l2_protocol_game::server::enterworld::OtherShortcut::Recipe,
                slot: 5,
                id: 77
            },
            ShortcutEntry::Other {
                kind: l2_protocol_game::server::enterworld::OtherShortcut::Bookmark,
                slot: 6,
                id: 8
            },
            ShortcutEntry::Empty { slot: 8 },
        ]
    );
}

#[test]
fn macros_are_sent_one_packet_each_or_one_empty_packet() {
    let log = Log::default();
    let mut w = world(&log);
    enter_player(&mut w, &log, 1, equipped(1, "Hero"));
    let packets = log.packets(1);
    let empty: MacroList = decode(first(&packets, "MACRO_LIST"));
    assert_eq!((empty.revision, empty.count, empty.macro_entry), (2, 0, None));

    let mut p = equipped(2, "Two");
    p.profile.macros = vec![
        MacroData {
            id: 1000,
            icon: 3,
            name: "One".into(),
            descr: "d".into(),
            acronym: "O".into(),
            commands: vec![MacroCmdData {
                kind: 1,
                d1: 1177,
                d2: 2,
                cmd: String::new(),
            }],
        },
        MacroData {
            id: 1001,
            icon: 4,
            name: "Two".into(),
            descr: String::new(),
            acronym: "T".into(),
            commands: vec![],
        },
    ];
    enter_player(&mut w, &log, 2, p);
    let macros: Vec<MacroList> = log
        .packets(2)
        .iter()
        .filter(|p| name_of(p) == "MACRO_LIST")
        .map(|p| decode(p))
        .collect();
    assert_eq!(macros.len(), 2);
    assert!(macros.iter().all(|m| m.count == 2 && m.revision == 2));
    assert_eq!(macros[0].macro_entry.as_ref().unwrap().commands[0].d1, 1177);
    assert_eq!(macros[1].macro_entry.as_ref().unwrap().id, 1001);
}

#[test]
fn small_packets_carry_the_configuration_and_the_player() {
    let log = Log::default();
    let mut w = world(&log);
    w.set_settings(WorldSettings {
        warehouse_slots_clan: 200,
        ..WorldSettings::default()
    });
    let mut p = equipped(1, "Dwarf");
    p.race = 4;
    p.profile.recom_left = 20;
    p.profile.recom_time_left_ms = 3_600_000;
    p.profile.recom_have = 12;
    p.profile.death_penalty_level = 2;
    enter_player(&mut w, &log, 1, p);
    let packets = log.packets(1);
    let storage: ExStorageMaxCount = decode(first(&packets, "EX_STORAGE_MAX_COUNT"));
    assert_eq!(
        storage,
        ExStorageMaxCount {
            inventory: 100,
            warehouse: 120,
            clan: 200,
            private_sell: 4,
            private_buy: 5,
            dwarven_recipe: 50,
            common_recipe: 50,
            inventory_extra_slots: 0,
            inventory_quest_items: 100,
        }
    );
    let etc: EtcStatusUpdate = decode(first(&packets, "ETC_STATUS_UPDATE"));
    assert_eq!(etc.death_penalty_level, 2);
    assert_eq!(etc.weight_penalty, 0);
    // The hourglass runs: 3600 s left, active (type 0), a bonus for 12 recommendations at level 30.
    let votes: Vec<ExVoteSystemInfo> = packets
        .iter()
        .filter(|p| name_of(p) == "EX_VOTE_SYSTEM_INFO")
        .map(|p| decode(p))
        .collect();
    assert_eq!(votes.len(), 2);
    assert_eq!(votes[0], votes[1]);
    assert_eq!(
        votes[0],
        ExVoteSystemInfo {
            recom_left: 20,
            recom_have: 12,
            bonus_time: 3_600,
            // level 30 -> row 3; (12 - 1) / 10 = 1 -> 20
            bonus_value: 20,
            bonus_type: 0,
        }
    );
    let cool: SkillCoolTime = decode(first(&packets, "SKILL_COOL_TIME"));
    assert!(cool.entries.is_empty());
}

#[test]
fn weight_penalty_levels_follow_refresh_overloaded() {
    use l2_world::enter::weight_penalty;
    assert_eq!(weight_penalty(0, 0), 0);
    assert_eq!(weight_penalty(499, 1000), 0);
    assert_eq!(weight_penalty(500, 1000), 1);
    assert_eq!(weight_penalty(665, 1000), 1);
    assert_eq!(weight_penalty(666, 1000), 2);
    assert_eq!(weight_penalty(799, 1000), 2);
    assert_eq!(weight_penalty(800, 1000), 3);
    assert_eq!(weight_penalty(999, 1000), 3);
    assert_eq!(weight_penalty(1000, 1000), 4);
    assert_eq!(weight_penalty(5000, 1000), 4);
}

#[test]
fn ui_key_mapping_is_the_stored_bytes() {
    let mut p = player(1, "acc", "Hero");
    assert!(l2_world::enter::ui_key_mapping(&p).is_empty());
    // Signed decimal numbers followed by a tab; a bad number is 0, trailing empties are dropped.
    p.profile
        .variables
        .insert("UI_KEY_MAPPING".into(), "1\t-1\t127\tx\t\t".into());
    assert_eq!(l2_world::enter::ui_key_mapping(&p), [1, 255, 127, 0]);
    let off = WorldSettings {
        store_ui_settings: false,
        ..WorldSettings::default()
    };
    assert!(l2_world::enter::ui_setting(&p, &off).is_none());
    assert_eq!(
        l2_world::enter::ui_setting(&p, &WorldSettings::default()),
        Some(ExUiSetting {
            ui_key_mapping: vec![1, 255, 127, 0]
        })
    );
}

#[test]
fn nevit_points_come_from_the_player_variables() {
    let log = Log::default();
    let mut w = world(&log);
    let mut p = equipped(1, "Hero");
    p.profile.variables.insert("hunting_points".into(), "2000".into());
    p.profile.variables.insert("hunting_time".into(), "99999".into());
    // A real last logout: with `lastAccess` 0 Java's daily reset (milliseconds compared with
    // seconds, see `nevit_on_player_login`) clears the time on every login after 06:30 local.
    p.profile.last_access = l2_util::time::now_millis();
    enter_player(&mut w, &log, 1, p);
    let packets = log.packets(1);
    let points: l2_protocol_game::server::ExNevitAdventPointInfo =
        decode(first(&packets, "EX_NAVIT_ADVENT_POINT_INFO"));
    assert_eq!(points.points, 2000);
    let time: l2_protocol_game::server::ExNevitAdventTimeChange =
        decode(first(&packets, "EX_NAVIT_ADVENT_TIME_CHANGE"));
    // Paused, and capped like Java's constructor.
    assert_eq!((time.started, time.time), (false, 14_400));
}

// ---- requests answered by the world -----------------------------------------------------------

#[test]
fn net_ping_skill_cool_time_and_key_mapping_are_answered() {
    let log = Log::default();
    let mut w = world(&log);
    enter_player(&mut w, &log, 1, equipped(1, "Hero"));
    log.take();
    for input in [
        ClientInput::NetPing,
        ClientInput::SkillCoolTime,
        ClientInput::KeyMapping,
    ] {
        w.handle(WorldCommand::Input {
            session: SessionId(1),
            input,
        });
    }
    let packets = log.packets(1);
    assert_eq!(names(&packets), ["NET_PING", "SKILL_COOL_TIME", "EX_UI_SETTING"]);
    let ping: NetPing = decode(&packets[0]);
    assert!((0..1440).contains(&ping.game_time));
    // Unknown packets are ignored.
    w.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Other {
            opcode: 0x0A,
            ex_opcode: None,
        },
    });
    assert!(log.take().is_empty());
    assert_eq!(w.online_count(), 1);
}

// ---- chat -------------------------------------------------------------------------------------

fn two_players() -> (World, Log) {
    let log = Log::default();
    let mut w = world(&log);
    enter_player(&mut w, &log, 1, equipped(1, "Alice"));
    enter_player(&mut w, &log, 2, equipped(2, "Bob"));
    log.take();
    (w, log)
}

fn say_packet(from: i32, chat_type: ChatType, name: &str, text: &str) -> Vec<u8> {
    CreatureSay {
        sender_object_id: from,
        chat_type,
        sender: SenderField::Name(name.into()),
        message_id: -1,
        body: SayBody::Text(text.into()),
    }
    .to_bytes()
}

#[test]
fn general_shout_and_trade_chat_reach_everybody_once() {
    let (mut w, log) = two_players();
    for (chat_type, kind) in [(0, ChatType::General), (1, ChatType::Shout), (8, ChatType::Trade)] {
        say(&mut w, 1, chat_type, "hi", None);
        let out = log.take();
        let expected = say_packet(1, kind, "Alice", "hi");
        // Java ChatGeneral: the players in range first, then the speaker. Shout and trade go over
        // `World.getPlayers()` (the speaker included, in id order here).
        let order = if kind == ChatType::General {
            vec![Out::Packet(2, expected.clone()), Out::Packet(1, expected)]
        } else {
            vec![Out::Packet(1, expected.clone()), Out::Packet(2, expected)]
        };
        assert_eq!(out, order, "{kind:?}");
    }
}

#[test]
fn chat_level_limits_and_modes() {
    let log = Log::default();
    let mut w = world(&log);
    w.set_settings(WorldSettings {
        minimum_chat_level: 40,
        ..WorldSettings::default()
    });
    let mut low = player(1, "a", "Low");
    low.level = 19;
    let mut gm = player(2, "b", "Gm");
    gm.level = 25;
    gm.access_level = 100;
    enter_player(&mut w, &log, 1, low);
    enter_player(&mut w, &log, 2, gm);
    log.take();

    say(&mut w, 1, 0, "hi", None);
    assert_eq!(
        log.take(),
        vec![Out::Packet(1, text("Players can use general chat after Lv. 40."))]
    );
    say(&mut w, 1, 1, "hi", None);
    assert_eq!(
        log.take(),
        vec![Out::Packet(1, text("Players can Shout after Lv. 40."))]
    );
    // Trade chat has its own fixed limit of 20.
    say(&mut w, 1, 8, "hi", None);
    assert_eq!(
        log.take(),
        vec![Out::Packet(1, text("Players can use Trade chat after Lv. 20."))]
    );
    // A GM is exempt.
    say(&mut w, 2, 0, "gm", None);
    assert_eq!(log.take().len(), 2);
    say(&mut w, 2, 8, "gm", None);
    assert_eq!(log.take().len(), 2);

    // GlobalChat = OFF: shout goes nowhere, not even to the speaker. GM: only for a GM.
    w.set_settings(WorldSettings {
        minimum_chat_level: 0,
        global_chat: ChatBroadcast::Off,
        trade_chat: ChatBroadcast::Gm,
        ..WorldSettings::default()
    });
    say(&mut w, 2, 1, "off", None);
    assert!(log.take().is_empty());
    // (A level 19 player gets the level message before the mode is looked at.)
    say(&mut w, 1, 8, "gm only", None);
    assert_eq!(log.take().len(), 1);
    let mut normal = player(3, "c", "Normal");
    normal.level = 50;
    enter_player(&mut w, &log, 3, normal);
    log.take();
    say(&mut w, 3, 8, "gm only", None);
    assert!(
        log.take().is_empty(),
        "TradeChat = GM: a normal player is not heard"
    );
    say(&mut w, 2, 8, "gm only", None);
    assert_eq!(log.take().len(), 3);
    w.set_settings(WorldSettings {
        minimum_chat_level: 0,
        global_chat: ChatBroadcast::Global,
        ..WorldSettings::default()
    });
    say(&mut w, 1, 1, "global", None);
    assert_eq!(log.take().len(), 3);
}

#[test]
fn whispers_go_to_the_named_player_ignoring_case() {
    let (mut w, log) = two_players();
    say(&mut w, 1, 2, "psst", Some("bOB"));
    assert_eq!(
        log.take(),
        vec![
            Out::Packet(2, say_packet(1, ChatType::Whisper, "Alice", "psst")),
            Out::Packet(1, say_packet(1, ChatType::Whisper, "->Bob", "psst")),
        ]
    );
    say(&mut w, 1, 2, "psst", Some("Nobody"));
    assert_eq!(
        log.take(),
        vec![Out::Packet(
            1,
            SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline).to_bytes()
        )]
    );
    // No target: nothing happens.
    say(&mut w, 1, 2, "psst", None);
    assert!(log.take().is_empty());
}

#[test]
fn hero_voice_is_refused_for_a_normal_player() {
    let (mut w, log) = two_players();
    say(&mut w, 1, 17, "voice", None);
    assert_eq!(
        log.take(),
        vec![Out::Packet(1, text("Only Heroes can enter the Hero channel."))]
    );
}

#[test]
fn channels_without_a_handler_are_dropped() {
    let (mut w, log) = two_players();
    for chat_type in [4, 9, 14] {
        say(&mut w, 1, chat_type, "party?", None);
    }
    assert!(log.take().is_empty());
}

/// The party channel has a handler (M6-PARTY-01): without a party the speaker is told so.
#[test]
fn party_chat_without_a_party_says_so() {
    let (mut w, log) = two_players();
    say(&mut w, 1, 3, "party?", None);
    assert_eq!(log.take(), vec![Out::Packet(1, text("You are not in a party."))]);
}

#[test]
fn text_limits_follow_say2() {
    let (mut w, log) = two_players();
    let too_long = "x".repeat(106);
    say(&mut w, 1, 0, &too_long, None);
    let out = log.take();
    assert_eq!(out.len(), 1);
    let Out::Packet(1, packet) = &out[0] else {
        panic!("{out:?}")
    };
    assert_eq!(name_of(packet), "SYSTEM_MESSAGE");
    // 105 is the limit.
    say(&mut w, 1, 0, &"x".repeat(105), None);
    assert_eq!(log.take().len(), 2);

    // A text with an item link may be 500 long; the link must name an item of the speaker.
    let link = |id: i32, filler: usize| {
        format!(
            "\u{8}Type=1 ID={id} Color=0 Underline=0 Title=\u{1B}{}\u{1B}\u{8}",
            "y".repeat(filler)
        )
    };
    say(&mut w, 1, 0, &link(0x2000_0004, 200), None);
    assert_eq!(log.take().len(), 2, "owned item");
    say(&mut w, 1, 0, &link(0x2000_0004, 600), None);
    assert_eq!(names(&log.packets(1)), ["SYSTEM_MESSAGE"], "over 500");
    say(&mut w, 1, 0, &link(12345, 1), None);
    assert!(log.take().is_empty(), "somebody else's item");
    say(&mut w, 1, 0, "\u{8}no id here\u{8}", None);
    assert!(log.take().is_empty(), "no ID=");
    say(&mut w, 1, 0, "\u{8}Type=1 ID=0x2000\u{8}", None);
    assert!(log.take().is_empty(), "no digits");
    say(&mut w, 1, 0, "\u{8}Type=1 ID=536870916 no closing mark", None);
    assert!(log.take().is_empty(), "missing ending tag");
    // A GM may say more.
    let log2 = Log::default();
    let mut w2 = world(&log2);
    let mut gm = player(1, "a", "Gm");
    gm.access_level = 100;
    enter_player(&mut w2, &log2, 1, gm);
    log2.take();
    say(&mut w2, 1, 0, &too_long, None);
    assert_eq!(log2.take().len(), 1);
}

#[test]
fn an_invalid_channel_or_empty_text_logs_the_player_out() {
    for (chat_type, text_) in [(99, "hi"), (0, "")] {
        let (mut w, log) = two_players();
        say(&mut w, 1, chat_type, text_, None);
        let out = log.take();
        // ActionFailed, then the usual way out of the world (store, LeaveWorld, close).
        assert_eq!(out[0], Out::Packet(1, ActionFailed.to_bytes()));
        assert!(matches!(out[1], Out::Persist(PersistCommand::StorePlayer(_))));
        assert!(matches!(
            out[2],
            Out::Persist(PersistCommand::PlayerLeft { logout: true, .. })
        ));
        // Java `deleteMe()` after `storeMe()`: both players forget each other (M3-WREG-01).
        assert_eq!(out[3], Out::Packet(1, DeleteObject { object_id: 2 }.to_bytes()));
        assert_eq!(out[4], Out::Packet(2, DeleteObject { object_id: 1 }.to_bytes()));
        assert_eq!(out[5], Out::Packet(1, LeaveWorld.to_bytes()));
        assert_eq!(out[6], Out::Left(1, LeaveReason::Logout));
        assert_eq!(w.online_count(), 1);
    }
}

// ---- leaving ----------------------------------------------------------------------------------

#[test]
#[allow(clippy::type_complexity)] // a table of (name, action, last packet, reason, logout)
fn every_way_out_stores_first_and_ends_with_its_own_packet() {
    let cases: [(&str, fn(&mut World), Option<Vec<u8>>, LeaveReason, bool); 5] = [
        (
            "logout",
            |w| {
                w.handle(WorldCommand::Input {
                    session: SessionId(1),
                    input: ClientInput::Logout,
                })
            },
            Some(LeaveWorld.to_bytes()),
            LeaveReason::Logout,
            true,
        ),
        (
            "restart",
            |w| {
                w.handle(WorldCommand::Input {
                    session: SessionId(1),
                    input: ClientInput::Restart,
                })
            },
            Some(RestartResponse { result: true }.to_bytes()),
            LeaveReason::Restart,
            false,
        ),
        (
            "disconnect",
            |w| {
                w.handle(WorldCommand::Disconnected {
                    session: SessionId(1),
                })
            },
            None,
            LeaveReason::Disconnected,
            true,
        ),
        (
            "kick",
            |w| {
                w.handle(WorldCommand::Kick {
                    account: "acc".into(),
                })
            },
            Some(SystemMessage::new(SystemMessageId::AnotherPersonHasLoggedInWithTheSameAccount).to_bytes()),
            LeaveReason::Kicked,
            true,
        ),
        (
            "shutdown",
            |w| {
                w.handle(WorldCommand::Shutdown {
                    done: Box::new(|| {}),
                })
            },
            Some(ServerClose.to_bytes()),
            LeaveReason::ServerShutdown,
            true,
        ),
    ];
    for (what, act, last_packet, reason, logout) in cases {
        let log = Log::default();
        let mut w = world(&log);
        enter_player(&mut w, &log, 1, equipped(1, "Hero"));
        log.take();
        act(&mut w);
        let out = log.take();
        let mut expected = vec!["store", "left"];
        if last_packet.is_some() {
            expected.push("packet");
        }
        expected.push("sink");
        let kinds: Vec<&str> = out
            .iter()
            .map(|o| match o {
                Out::Persist(PersistCommand::StorePlayer(_)) => "store",
                Out::Persist(PersistCommand::PlayerLeft { .. }) => "left",
                Out::Persist(PersistCommand::ItemChanges { .. } | PersistCommand::ItemRows { .. }) => "items",
                Out::Persist(PersistCommand::SaveGroundItems(_)) => "ground",
                Out::Persist(PersistCommand::SkillChanges { .. }) => "skills",
                Out::Persist(
                    PersistCommand::RecoValues { .. }
                    | PersistCommand::ResetRecommends
                    | PersistCommand::PlayerVariables { .. },
                ) => "reco",
                Out::Persist(
                    PersistCommand::StorePunishment(_)
                    | PersistCommand::EndPunishment { .. }
                    | PersistCommand::CharAccessLevel { .. }
                    | PersistCommand::RepairChar { .. },
                ) => "punishment",
                Out::Persist(PersistCommand::Summon(_)) => "summon",
                Out::Persist(PersistCommand::Clan(_)) => "clan",
                Out::Persist(PersistCommand::BuyList(_)) => "buylist",
                Out::Persist(PersistCommand::Recipe(_)) => "recipe",
                Out::Persist(PersistCommand::FishingChampionship(_)) => "fishing",
                Out::Persist(PersistCommand::PetRowRequest { .. }) => "pet row",
                Out::Persist(PersistCommand::Friend(_)) => "friend",
                Out::Persist(PersistCommand::Henna(_)) => "henna",
                Out::Persist(PersistCommand::ItemAuction(_) | PersistCommand::ItemAuctionRequest { .. }) => {
                    "item auction"
                }
                Out::Persist(PersistCommand::InstanceTime(_)) => "instance time",
                Out::Persist(PersistCommand::Quest(_)) => "quest",
                Out::Persist(PersistCommand::GlobalVariables(_)) => "global variables",
                Out::Persist(PersistCommand::Bookmark(_)) => "bookmark",
                Out::Persist(PersistCommand::BbsFavorite(_) | PersistCommand::BbsFavoriteRequest { .. }) => {
                    "community board"
                }
                Out::Persist(PersistCommand::Lottery(_) | PersistCommand::LotteryRequest { .. }) => "lottery",
                Out::Persist(PersistCommand::Contact(_)) => "contact",
                Out::Persist(PersistCommand::FriendSummariesRequest { .. }) => "sync read",
                Out::Persist(PersistCommand::Mail(_)) => "mail",
                Out::Persist(
                    PersistCommand::MailAttachmentsRequest { .. }
                    | PersistCommand::ClanWarehouseRequest { .. }
                    | PersistCommand::CrestIdsRequest { .. }
                    | PersistCommand::BlockListRequest { .. }
                    | PersistCommand::OfflinePetRowsRequest { .. },
                ) => "sync read",
                Out::Persist(
                    PersistCommand::Residence(_) | PersistCommand::ResidenceRereadRequest { .. },
                ) => "residence",
                Out::Persist(PersistCommand::BotReport(_)) => "bot report",
                Out::Persist(PersistCommand::OfflineTrade(_)) => "offline trade",
                Out::Persist(PersistCommand::Manor(_)) => "manor",
                Out::Persist(
                    PersistCommand::Boss(_)
                    | PersistCommand::BossRereadRequest { .. }
                    | PersistCommand::GlobalVariablesRereadRequest { .. },
                ) => "boss",
                Out::Persist(PersistCommand::Cursed(_)) => "cursed",
                Out::Persist(PersistCommand::Olympiad(_) | PersistCommand::OlympiadRereadRequest { .. }) => {
                    "olympiad"
                }
                Out::Persist(PersistCommand::Minigame(_)) => "minigame",
                Out::Persist(PersistCommand::Vehicle(_)) => "vehicle",
                Out::Persist(
                    PersistCommand::SevenSigns(_) | PersistCommand::SevenSignsReadRequest { .. },
                ) => "seven signs",
                Out::Persist(PersistCommand::Subclass(_) | PersistCommand::ClassIndexRequest { .. }) => {
                    "subclass"
                }
                Out::Persist(
                    PersistCommand::Announcement(_)
                    | PersistCommand::PetitionFeedback { .. }
                    | PersistCommand::Wedding(_),
                ) => "admin",
                Out::Packet(..) => "packet",
                Out::Left(..) => "sink",
                Out::Ls(_) => "ls",
            })
            .collect();
        // The kick message is sent before the store (Java sends it with `storeAndDeleteWith`,
        // after the store; the order of an already queued packet does not matter to the client).
        if what == "kick" {
            assert_eq!(kinds, ["packet", "store", "left", "sink"], "{what}");
        } else {
            assert_eq!(kinds, expected, "{what}");
        }
        if let Some(packet) = last_packet {
            assert!(out.contains(&Out::Packet(1, packet)), "{what}");
        }
        assert!(
            out.iter().any(|o| matches!(
                o,
                Out::Persist(PersistCommand::PlayerLeft { account, object_id, logout: l })
                    if account == "acc" && *object_id == ObjectId(1) && *l == logout
            )),
            "{what}"
        );
        assert_eq!(out.last(), Some(&Out::Left(1, reason)), "{what}");
        assert_eq!(w.online_count(), 0);
    }
}

// ---- hennas (Java HennaInfo, Player.getHennaStat*) ---------------------------------------------

/// Three dyes for class 10: 1 is STR+1 CON-3 (the first retail dye), 2 is STR+4 and 3 is INT-1
/// DEX+2.
fn henna_data() -> FakeData {
    let mut data = data();
    let dye = |id: i32, item: i32, stats: &str| {
        format!(
            r#"<henna dyeId="{id}" dyeItemId="{item}" dyeLevel="1"><stats {stats} /><wear count="10" fee="37000" /><cancel count="5" fee="7400" /><classId>10</classId></henna>"#
        )
    };
    let xml = format!(
        "<list>{}{}{}</list>",
        dye(1, 4445, r#"str="1" con="-3""#),
        dye(2, 4446, r#"str="4""#),
        dye(3, 4447, r#"int="-1" dex="2""#),
    );
    data.stats.hennas = Arc::new(l2_data::Hennas::from_xml(&xml).unwrap());
    data
}

fn world_with_dyes(log: &Log) -> World {
    let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
    world.set_static_data(Box::new(henna_data()));
    world
}

/// The `HennaInfo` the character gets at enter world, decoded and as sent.
fn henna_info_at_enter(hennas: [i32; 3]) -> (HennaInfo, Vec<u8>) {
    let log = Log::default();
    let mut w = world_with_dyes(&log);
    let mut p = equipped(1, "Hero");
    p.profile.hennas = hennas;
    enter_player(&mut w, &log, 1, p);
    let packets = log.packets(1);
    let bytes = first(&packets, "HENNA_INFO").clone();
    (decode(&bytes), bytes)
}

#[test]
fn henna_info_without_dyes_is_the_empty_packet() {
    let (info, _) = henna_info_at_enter([0, 0, 0]);
    assert_eq!(info, HennaInfo::default());
}

#[test]
fn henna_info_with_one_dye_has_its_bonus_and_the_java_wire_form() {
    let (info, bytes) = henna_info_at_enter([1, 0, 0]);
    assert_eq!(
        info,
        HennaInfo {
            str: 1,
            con: -3,
            dye_ids: vec![1],
            ..HennaInfo::default()
        }
    );
    // HennaInfo.writeImpl: the opcode, INT STR CON MEN DEX WIT as bytes, then 3 slots, the count,
    // and each dye id followed by a 1.
    let mut want = vec![0xE5, 0, 1, 0xFD, 0, 0, 0];
    for word in [3_i32, 1, 1, 1] {
        want.extend_from_slice(&word.to_le_bytes());
    }
    assert_eq!(bytes, want);
}

#[test]
fn henna_info_with_three_dyes_adds_the_bonuses_with_the_java_cap() {
    let (info, _) = henna_info_at_enter([1, 2, 3]);
    // STR: 1, then 1 + 4 = 5 (not above 5, so it counts). CON -3. INT -1 and DEX +2 from dye 3.
    assert_eq!(
        info,
        HennaInfo {
            int: -1,
            str: 5,
            con: -3,
            dex: 2,
            dye_ids: vec![1, 2, 3],
            ..HennaInfo::default()
        }
    );
}

#[test]
fn a_dye_the_datapack_does_not_know_is_an_empty_slot() {
    let (info, _) = henna_info_at_enter([1, 99, 0]);
    assert_eq!(
        info,
        HennaInfo {
            str: 1,
            con: -3,
            dye_ids: vec![1],
            ..HennaInfo::default()
        }
    );
}

#[test]
fn a_dye_changes_the_str_and_con_that_user_info_shows() {
    let user_info = |hennas: [i32; 3]| {
        let log = Log::default();
        let mut w = world_with_dyes(&log);
        let mut p = equipped(1, "Hero");
        p.profile.hennas = hennas;
        enter_player(&mut w, &log, 1, p);
        let packets = log.packets(1);
        let info: UserInfo = decode(first(&packets, "USER_INFO"));
        info
    };
    let plain = user_info([0, 0, 0]);
    let dyed = user_info([1, 0, 0]);
    // The dye is STR+1 CON-3 and the engine applies it (`FuncHenna`); nothing else moves.
    assert_eq!((dyed.str, dyed.con), (plain.str + 1, plain.con - 3));
    assert_eq!(
        (dyed.dex, dyed.int, dyed.wit, dyed.men),
        (plain.dex, plain.int, plain.wit, plain.men)
    );
}
