#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! `//skill_test <skillId>` (`admin/skill_test.rs`): the cast of a level 1 skill shown toward the
//! GM from the target (or the GM).
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminTest.java

use std::sync::{Arc, Mutex};

use l2_data::skill::{EnchantSkillGroups, SkillLoadOptions, SkillTemplates};
use l2_data::{ItemTemplates, PlayerTemplates};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{MagicSkillUse, SystemMessage, SystemMessageParam};
use l2_world::*;

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}

    fn ls_event(&mut self, _event: LsEvent) {}
}

fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [
        (0, 0),
        (30, 0),
        (40, 30),
        (50, 40),
        (60, 50),
        (70, 60),
        (100, 70),
    ] {
        rights.add_level(level, child);
    }
    rights.add_command("skill_test", 100, false);
    rights
}

struct Fake {
    rights: AdminRights,
    stats: StatData,
}

impl StaticData for Fake {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level >= 70 {
            AccessLevelStatic {
                name_color: 0,
                title_color: 0,
                is_gm: true,
            }
        } else {
            AccessLevelStatic::USER
        }
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

/// One skill, "Reflect" (3100), as the other admin tests use.
const SKILLS: &str = r##"<list>
<skill id="3100" levels="1" name="Reflect"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="reflectDam" val="50"/></effect></effects></skill>
</list>"##;

fn stat_data() -> StatData {
    let mut levels = String::new();
    for level in 1..=40 {
        levels.push_str(&format!(
            "<level val=\"{level}\"><hp>{level}</hp><mp>{level}</mp><cp>{level}</cp></level>"
        ));
    }
    let xml = format!(
        r#"<list><classId>0</classId><staticData>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{levels}</lvlUpgainData></list>"#
    );
    StatData {
        player_templates: Arc::new(PlayerTemplates::from_xml(&xml, 85).unwrap()),
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
        ..StatData::default()
    }
}

const GM: i32 = 100;
const P1: i32 = 1;
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

fn player(id: i32, access_level: i32, name: &str) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: name.to_owned(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 40,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level,
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 10.0,
        max_cp: 10.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct Setup {
    world: World,
    log: Log,
}

fn setup() -> Setup {
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(6), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake {
        rights: rights(),
        stats: stat_data(),
    }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let mut s = Setup { world, log };
    s.enter(GM, 100, "Gamemaster");
    s.enter(P1, 0, "Plain");
    s.log.take();
    s
}

impl Setup {
    fn enter(&mut self, id: i32, access_level: i32, name: &str) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player(id, access_level, name)),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn cmd(&mut self, command: &str) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(GM).unwrap()),
            input: ClientInput::BuildCmd {
                command: command.to_owned(),
            },
        });
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn texts(out: &Sent) -> Vec<String> {
    out.iter()
        .filter(|(_, b)| name_of(b) == ServerOpcode::SystemMessage.name())
        .filter_map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            let m = SystemMessage::decode(&mut PacketReader::new(&b[skip..])).ok()?;
            match m.params.as_slice() {
                [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
                _ => None,
            }
        })
        .collect()
}

fn casts(out: &Sent) -> Vec<MagicSkillUse> {
    out.iter()
        .filter(|(_, b)| name_of(b) == ServerOpcode::MagicSkillUse.name())
        .map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            MagicSkillUse::decode(&mut PacketReader::new(&b[skip..])).unwrap()
        })
        .collect()
}

const USAGE: &str = "Command format is //skill_test <ID>";

#[test]
fn skill_test_without_an_id_is_the_usage() {
    let mut s = setup();
    s.cmd("skill_test");
    s.cmd("skill_test abc");
    assert_eq!(texts(&s.log.take()), vec![USAGE.to_owned(), USAGE.to_owned()]);
}

#[test]
fn skill_test_without_a_target_is_the_usage() {
    let mut s = setup();
    s.cmd("skill_test 3100");
    assert_eq!(texts(&s.log.take()), vec![USAGE.to_owned()]);
}

#[test]
fn skill_test_shows_the_cast_of_the_target_toward_the_gm() {
    let mut s = setup();
    s.world.set_target(ObjectId(GM), Some(ObjectId(P1)));
    s.cmd("skill_test 3100");
    let out = s.log.take();
    let shown = casts(&out);
    assert_eq!(shown.len(), 2, "seen by the GM and the target");
    let cast = &shown[0];
    assert_eq!(cast.creature_id, P1, "the target casts");
    assert_eq!(cast.target_id, GM, "toward the GM");
    assert_eq!(cast.skill_id, 3100);
    assert_eq!(cast.skill_level, 1);
    // The caster now targets the GM (Java `caster.setTarget(activeChar)`).
    assert_eq!(s.world.target_of(ObjectId(P1)), Some(ObjectId(GM)));
}

#[test]
fn skill_test_of_an_unknown_skill_shows_nothing() {
    let mut s = setup();
    s.world.set_target(ObjectId(GM), Some(ObjectId(P1)));
    s.cmd("skill_test 99999");
    assert!(casts(&s.log.take()).is_empty());
}
