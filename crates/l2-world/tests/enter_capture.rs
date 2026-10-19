#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The packets the world sends when a character enters, compared with a recorded Java game
//! server session (`fixtures/captures/java-gs-session.jsonl`, scrubbed, committed, so this runs
//! in CI too). `L2_CAPTURE_DIR` points at the owner's own recordings instead.
//!
//! The character of the recording is rebuilt from the packets Java sent for it (its `UserInfo`,
//! `ItemList`, `SkillList`, `ShortcutInit`), the world is asked to enter it, and the packets
//! that come out must have the same order and, where nothing but the character decides them,
//! the same bytes. The stats of `UserInfo` come from the stat engine over the few datapack
//! entries the character uses (`capture_stat_data`), so they are checked against Java too.
//!
//! What the recording has and the world does not send yet, with the reason:
//! - `NPC_INFO` (57 of them): visibility, M3
//! - `SYSTEM_MESSAGE` for the zone and for Seven Signs, `EX_SET_COMPASS_ZONE_CODE`: zones, Seven
//!   Signs
//! - `SAY2` x2: the server announcements (`announcements.xml`), not ported
//! - `SHOW_TUTORIAL_MARK`, `PLAY_SOUND`, `TUTORIAL_ENABLE_CLIENT_EVENT`: the tutorial quest
//!   script of a new character, scripts come last (D-05)
//! - `STATUS_UPDATE`, `MOVE_TO_LOCATION`, `ABNORMAL_STATUS_UPDATE`: regeneration, the tutorial
//!   camera and effects, M3/M4
//! - `QUEST_LIST` carries the tutorial quest in the recording; the world sends an empty list
//!   (quests are not loaded)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::enterworld::{OtherShortcut, ShortcutEntry, ShortcutInit, SkillList};
use l2_protocol_game::server::{ItemList, SystemMessage, UserInfo};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::*;

#[derive(Debug, Clone)]
struct Record {
    dir: String,
    name: String,
    raw: Vec<u8>,
}

fn capture_path() -> PathBuf {
    let dir = std::env::var_os("L2_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures"));
    dir.join("java-gs-session.jsonl")
}

/// The game packets of the first connection that sent `ENTER_WORLD`.
fn load() -> Option<Vec<Record>> {
    let path = capture_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("skipped: {} is not readable ({e})", path.display());
            return None;
        }
    };
    let mut by_conn: Vec<(u64, Vec<Record>)> = Vec::new();
    for line in text.lines().skip(1) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["channel"] != "game" {
            continue;
        }
        let (Some(conn), Some(dir), Some(name), Some(raw)) = (
            v["conn"].as_u64(),
            v["dir"].as_str(),
            v["name"].as_str(),
            v["raw"].as_str(),
        ) else {
            continue;
        };
        let record = Record {
            dir: dir.into(),
            name: name.into(),
            raw: hex::decode(raw).unwrap(),
        };
        match by_conn.iter_mut().find(|(c, _)| *c == conn) {
            Some((_, list)) => list.push(record),
            None => by_conn.push((conn, vec![record])),
        }
    }
    by_conn
        .into_iter()
        .map(|(_, list)| list)
        .find(|list| list.iter().any(|r| r.dir == "c2s" && r.name == "ENTER_WORLD"))
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn s2c<'a>(records: &'a [Record], name: &str) -> Vec<&'a Record> {
    records
        .iter()
        .filter(|r| r.dir == "s2c" && r.name == name)
        .collect()
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

/// Item templates as the recording shows them (what `ItemList` says about each item).
#[derive(Clone)]
struct CaptureData {
    items: HashMap<i32, ItemStatic>,
    passive: Vec<i32>,
    stats: StatData,
    exp_percent: f64,
}

impl StaticData for CaptureData {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        self.items.get(&item_id).cloned()
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.passive.contains(&skill_id)
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        self.exp_percent
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

/// The datapack parts the stat engine reads for the recorded character, written out as the
/// XML of the real datapack (only the entries this character uses), so the test runs without
/// the reference folders: the Elven Mystic class template, the items it wears and the
/// `statBonus.xml` rows of its base stats. With them, every number of `UserInfo` comes from the
/// stat engine and must equal what Java sent.
fn capture_stat_data() -> StatData {
    // `data/stats/items/00000-00099.xml` and `00400-00499.xml`, only the stat-related lines.
    let items = l2_data::ItemTemplates::from_xml(
        r#"<list>
	<item id="6" type="Weapon" name="Apprentice's Wand">
		<set name="weapon_type" val="BLUNT" />
		<set name="bodypart" val="rhand" />
		<set name="is_magic_weapon" val="true" />
		<stats>
			<stat type="pAtk">5</stat>
			<stat type="mAtk">7</stat>
			<stat type="critRate">4</stat>
			<stat type="accCombat">4.75</stat>
			<stat type="pAtkSpd">379</stat>
			<stat type="randomDamage">20</stat>
			<stat type="pAtkRange">40</stat>
		</stats>
	</item>
	<item id="425" type="Armor" name="Apprentice's Tunic">
		<set name="armor_type" val="MAGIC" />
		<set name="bodypart" val="chest" />
		<stats>
			<stat type="pDef">17</stat>
			<stat type="maxMp">19</stat>
		</stats>
	</item>
	<item id="461" type="Armor" name="Apprentice's Stockings">
		<set name="armor_type" val="MAGIC" />
		<set name="bodypart" val="legs" />
		<stats>
			<stat type="pDef">10</stat>
			<stat type="maxMp">10</stat>
		</stats>
	</item>
</list>"#,
    )
    .unwrap();
    // `data/stats/players/templates/StartingClass/ElvenMystic.xml`, level 1 only.
    let templates = l2_data::PlayerTemplates::from_xml(
        r#"<list>
	<classId>25</classId>
	<staticData>
		<baseINT>37</baseINT><baseSTR>21</baseSTR><baseCON>25</baseCON>
		<baseMEN>40</baseMEN><baseDEX>24</baseDEX><baseWIT>23</baseWIT>
		<basePAtk>3</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
		<basePDef><chest>15</chest><legs>8</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
		<baseMAtk>6</baseMAtk>
		<baseMDef><rear>9</rear><lear>9</lear><rfinger>5</rfinger><lfinger>5</lfinger><neck>13</neck></baseMDef>
		<baseAtkRange>20</baseAtkRange>
		<baseMoveSpd><walk>85</walk><run>122</run><slowSwim>50</slowSwim><fastSwim>50</fastSwim></baseMoveSpd>
		<collisionMale><radius>7.5</radius><height>24</height></collisionMale>
		<collisionFemale><radius>7.5</radius><height>23</height></collisionFemale>
	</staticData>
	<lvlUpgainData>
		<level val="1"><hp>104.0</hp><mp>40.0</mp><cp>52.0</cp></level>
	</lvlUpgainData>
</list>"#,
        85,
    )
    .unwrap();
    // `data/stats/statBonus.xml`, the rows of the character's base stats.
    let mut bonus = BaseStatBonus::loaded();
    for (stat, value, b) in [
        (BaseStat::Str, 21, 0.61),
        (BaseStat::Int, 37, 1.12),
        (BaseStat::Dex, 24, 1.04),
        (BaseStat::Wit, 23, 1.16),
        (BaseStat::Con, 25, 0.93),
        (BaseStat::Men, 40, 1.49),
    ] {
        assert!(bonus.set(stat, value, b));
    }
    StatData {
        items: Arc::new(items),
        player_templates: Arc::new(templates),
        base_stat_bonus: Arc::new(bonus),
        ..StatData::default()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(Vec<u8>),
    Left(LeaveReason),
}

struct Sink(Arc<Mutex<Vec<Out>>>);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.0.lock().unwrap().push(Out::Packet(bytes));
    }
    fn left_world(&self, reason: LeaveReason) {
        self.0.lock().unwrap().push(Out::Left(reason));
    }
}

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

/// Everything from the first USER_INFO that decides what the world sends.
struct Rebuilt {
    player: PlayerData,
    data: CaptureData,
    first_info: UserInfo,
}

fn rebuild(records: &[Record]) -> Rebuilt {
    let first_info: UserInfo = decode(&s2c(records, "USER_INFO")[0].raw);
    let item_list: ItemList = decode(&s2c(records, "ITEM_LIST")[0].raw);
    let skills: SkillList = decode(&s2c(records, "SKILL_LIST")[0].raw);
    let shortcuts: ShortcutInit = decode(&s2c(records, "SHORT_CUT_INIT")[0].raw);
    let rotation = &s2c(records, "EX_ROTATION")[0].raw;
    let heading = i32::from_le_bytes(rotation[7..11].try_into().unwrap());
    let vote = &s2c(records, "EX_VOTE_SYSTEM_INFO")[0].raw;
    let recom_left = i32::from_le_bytes(vote[3..7].try_into().unwrap());
    let recom_have = i32::from_le_bytes(vote[7..11].try_into().unwrap());

    let mut reuse_groups: HashMap<i32, i32> = HashMap::new();
    let mut shortcut_data = Vec::new();
    for entry in &shortcuts.shortcuts {
        let (wire_slot, kind, shortcut_id, level) = match *entry {
            ShortcutEntry::Empty { slot } => (slot, 0, 0, 0),
            ShortcutEntry::Item {
                slot,
                id,
                shared_reuse_group,
            } => {
                reuse_groups.insert(id, shared_reuse_group);
                (slot, 1, id, 0)
            }
            ShortcutEntry::Skill { slot, id, level } => (slot, 2, id, level),
            ShortcutEntry::Other { kind, slot, id } => (
                slot,
                match kind {
                    OtherShortcut::Action => 3,
                    OtherShortcut::Macro => 4,
                    OtherShortcut::Recipe => 5,
                    OtherShortcut::Bookmark => 6,
                },
                id,
                0,
            ),
        };
        shortcut_data.push(ShortcutData {
            slot: wire_slot % 12,
            page: wire_slot / 12,
            kind,
            shortcut_id,
            level,
        });
    }

    let mut items = Vec::new();
    let mut templates = HashMap::new();
    for i in &item_list.items {
        items.push(ItemData {
            object_id: ObjectId(i.object_id),
            item_id: i.item_id,
            count: i.count,
            enchant_level: i32::from(i.enchant),
            loc: if i.equipped == 1 { "PAPERDOLL" } else { "INVENTORY" }.into(),
            loc_data: i.location,
            custom_type1: i32::from(i.custom_type1),
            custom_type2: i32::from(i.custom_type2),
            mana_left: i.mana,
            time: -1,
        });
        templates.insert(
            i.item_id,
            ItemStatic {
                display_id: i.item_id,
                type2: i.type2,
                body_part: i.body_part,
                quest_item: false,
                weight: 0,
                shared_reuse_group: reuse_groups.get(&i.object_id).copied().unwrap_or(-1),
                weapon: i.type2 == 0,
            },
        );
    }
    // The Java capture shows the equipped weapon in the right hand slot.
    assert!(
        items
            .iter()
            .any(|i| i.loc_data == paperdoll::RHAND as i32 && i.loc == "PAPERDOLL")
    );

    let player = PlayerData {
        object_id: ObjectId(first_info.object_id),
        account: "account".into(),
        name: first_info.name.clone(),
        title: first_info.title.clone(),
        class_id: first_info.class_id,
        base_class: first_info.base_class,
        race: first_info.race,
        sex: i32::from(first_info.is_female),
        face: first_info.face,
        hair_style: first_info.hair_style,
        hair_color: first_info.hair_color,
        level: first_info.level,
        exp: first_info.exp,
        sp: i64::from(first_info.sp),
        karma: first_info.karma,
        pvp_kills: first_info.pvp_kills,
        pk_kills: first_info.pk_kills,
        access_level: 0,
        x: first_info.x,
        y: first_info.y,
        z: first_info.z,
        heading,
        cur_hp: f64::from(first_info.current_hp),
        max_hp: f64::from(first_info.max_hp),
        cur_mp: f64::from(first_info.current_mp),
        max_mp: f64::from(first_info.max_mp),
        cur_cp: f64::from(first_info.current_cp),
        max_cp: f64::from(first_info.max_cp),
        online_time: 0,
        items,
        skills: skills
            .skills
            .iter()
            .map(|s| SkillData {
                skill_id: s.id,
                skill_level: s.level,
            })
            .collect(),
        shortcuts: shortcut_data,
        profile: PlayerProfile {
            fame: first_info.fame,
            vitality_points: first_info.vitality_points,
            recom_left,
            recom_have,
            ..PlayerProfile::default()
        },
    };
    let stats = capture_stat_data();
    Rebuilt {
        player,
        data: CaptureData {
            items: templates,
            passive: skills.skills.iter().filter(|s| s.passive).map(|s| s.id).collect(),
            stats,
            exp_percent: first_info.exp_percent,
        },
        first_info,
    }
}

/// The Java names that the world does not send yet (see the module comment).
const NOT_SENT: [&str; 10] = [
    "NPC_INFO",
    "EX_SET_COMPASS_ZONE_CODE",
    "SAY2",
    "SHOW_TUTORIAL_MARK",
    "PLAY_SOUND",
    "TUTORIAL_ENABLE_CLIENT_EVENT",
    "STATUS_UPDATE",
    "MOVE_TO_LOCATION",
    "ABNORMAL_STATUS_UPDATE",
    "EX_UI_SETTING",
];

/// The names Java sent after `ENTER_WORLD`, up to its `SKILL_LIST`, without what is not sent yet.
fn expected_names(records: &[Record]) -> Vec<String> {
    let start = records
        .iter()
        .position(|r| r.dir == "c2s" && r.name == "ENTER_WORLD")
        .unwrap();
    let mut names = Vec::new();
    for r in records[start + 1..].iter().filter(|r| r.dir == "s2c") {
        if NOT_SENT.contains(&r.name.as_str()) {
            continue;
        }
        if r.name == "SYSTEM_MESSAGE" {
            // Only the welcome message; the others are the zone and Seven Signs.
            let message: SystemMessage = decode(&r.raw);
            if message.id != SystemMessageId::WelcomeToTheWorldOfLineageIi {
                continue;
            }
        }
        names.push(r.name.clone());
        if r.name == "SKILL_LIST" {
            break;
        }
    }
    names
}

#[test]
fn the_world_sends_what_java_sent_for_the_recorded_character() {
    let Some(records) = load() else {
        return;
    };
    let rebuilt = rebuild(&records);
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut world = World::new(WorldRng::from_seed(1), Box::new(NullOutbox));
    world.set_static_data(Box::new(rebuilt.data.clone()));
    // The recorded server has `MaximumWarehouseSlotsForClan = 200`.
    world.set_settings(WorldSettings {
        warehouse_slots_clan: 200,
        ..WorldSettings::default()
    });
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(rebuilt.player.clone()),
        sink: Box::new(Sink(Arc::clone(&log))),
        ip: None,
    });
    let ours: Vec<Vec<u8>> = log
        .lock()
        .unwrap()
        .iter()
        .filter_map(|o| match o {
            Out::Packet(bytes) => Some(bytes.clone()),
            Out::Left(_) => None,
        })
        .collect();

    // 1. The order and kinds of the packets.
    let our_names: Vec<String> = ours.iter().map(|p| name_of(p)).collect();
    let java_names = expected_names(&records);
    assert_eq!(our_names, java_names);

    // 2. The bytes, packet by packet. `QUEST_LIST` holds the tutorial quest in the recording;
    //    `USER_INFO` is compared below.
    let mut compared = 0;
    let mut seen: HashMap<String, usize> = HashMap::new();
    for packet in &ours {
        let name = name_of(packet);
        let nth = seen.entry(name.clone()).or_insert(0);
        let mut java = s2c(&records, &name);
        if name == "SYSTEM_MESSAGE" {
            // The recording has more of them (zone, Seven Signs); ours is the welcome message.
            java.retain(|r| {
                decode::<SystemMessage>(&r.raw).id == SystemMessageId::WelcomeToTheWorldOfLineageIi
            });
        }
        let java = java[(*nth).min(java.len() - 1)];
        *nth += 1;
        if matches!(name.as_str(), "USER_INFO" | "QUEST_LIST") {
            continue;
        }
        assert_eq!(packet, &java.raw, "{name} #{nth} differs from Java");
        compared += 1;
    }
    assert!(compared >= 20, "{compared}");

    // 3. `USER_INFO`: the same character, apart from what changed in between in the recording.
    let infos: Vec<UserInfo> = ours
        .iter()
        .filter(|p| name_of(p) == "USER_INFO")
        .map(|p| decode(p))
        .collect();
    assert_eq!(infos.len(), 2);
    let java_infos = s2c(&records, "USER_INFO");
    let first: UserInfo = decode(&java_infos[0].raw);
    let second: UserInfo = decode(&java_infos[1].raw);
    assert_eq!(first, rebuilt.first_info);
    // Our items have no weight in this test; Java's inventory weighs 5210.
    let patch = |mut ours: UserInfo, java: &UserInfo| {
        assert_eq!(ours.current_load, 0);
        ours.current_load = java.current_load;
        ours
    };
    assert_eq!(patch(infos[0].clone(), &first), first);
    // The second one was sent after the character entered: no cloak, recommendations loaded;
    // regeneration had added some MP and CP by then.
    let mut second_without_regeneration = second.clone();
    second_without_regeneration.current_mp = first.current_mp;
    second_without_regeneration.current_cp = first.current_cp;
    assert_eq!(
        patch(infos[1].clone(), &second_without_regeneration),
        second_without_regeneration
    );
    assert!(!infos[1].can_equip_cloak && infos[0].can_equip_cloak);
    assert_eq!(second.recom_left, 20);

    // 4. The key mapping the client asks for before `ENTER_WORLD`.
    let key_mapping = s2c(&records, "EX_UI_SETTING");
    let log2 = Arc::new(Mutex::new(Vec::new()));
    let mut world2 = World::new(WorldRng::from_seed(1), Box::new(NullOutbox));
    world2.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(rebuilt.player),
        sink: Box::new(Sink(Arc::clone(&log2))),
        ip: None,
    });
    log2.lock().unwrap().clear();
    world2.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::KeyMapping,
    });
    assert_eq!(
        log2.lock().unwrap().as_slice(),
        [Out::Packet(key_mapping[0].raw.clone())]
    );
}

#[test]
fn the_encoders_reproduce_the_recorded_enter_world_packets() {
    let Some(records) = load() else {
        return;
    };
    // Every packet of the sequence we send decodes and encodes to the same bytes in the recording.
    let rebuilt = rebuild(&records);
    let ours = [
        rebuilt.first_info.to_bytes(),
        decode::<ItemList>(&s2c(&records, "ITEM_LIST")[0].raw).to_bytes(),
        decode::<SkillList>(&s2c(&records, "SKILL_LIST")[0].raw).to_bytes(),
    ];
    assert_eq!(ours[0], s2c(&records, "USER_INFO")[0].raw);
    assert_eq!(ours[1], s2c(&records, "ITEM_LIST")[0].raw);
    assert_eq!(ours[2], s2c(&records, "SKILL_LIST")[0].raw);
}
