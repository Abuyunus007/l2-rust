#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The shortcut bar (M5-SHORTCUT-01): `RequestShortcutReg` and `RequestShortcutDel` with the rules
//! of `Shortcuts.java`, the packets they cause, and the deletions of other actions (a removed
//! skill: `tests/admin_skills.rs`; a destroyed item: `tests/inventory.rs`; a level change:
//! `tests/learn.rs`). Expected behaviour comes from `RequestShortcutReg.java`,
//! `RequestShortcutDel.java`, `Shortcuts.java`, `ShortcutRegister.java` and `ShortcutInit.java`.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::enterworld::{OtherShortcut, ShortcutEntry};
use l2_protocol_game::server::{ExAutoSoulShot, ShortcutInit, ShortcutRegister};
use l2_util::WorldRng;
use l2_world::shortcuts::shortcut_register_packet;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

type Sent = Vec<Vec<u8>>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.0.0.lock().unwrap().push(bytes);
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

/// Like the real `DatapackData`: only etc items show their shared reuse group in a restored list.
struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap_or(5),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: if t.etc().is_some() {
                t.shared_reuse_group
            } else {
                -1
            },
            weapon: t.is_weapon(),
        })
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn names(sent: &Sent) -> Vec<String> {
    sent.iter().map(|b| name_of(b)).collect()
}

fn decoded<P: Decode>(sent: &Sent, name: &str) -> Vec<P> {
    sent.iter()
        .filter(|b| name_of(b) == name)
        .map(|raw| P::decode(&mut PacketReader::new(&raw[1..])).unwrap())
        .collect()
}

fn registers(sent: &Sent) -> Vec<ShortcutRegister> {
    decoded(sent, "SHORT_CUT_REGISTER")
}

fn inits(sent: &Sent) -> Vec<ShortcutInit> {
    decoded(sent, "SHORT_CUT_INIT")
}

fn auto_shot_packets(sent: &Sent) -> Vec<(i32, i32)> {
    sent.iter()
        .filter(|b| name_of(b) == "EX_AUTO_SOULSHOT")
        .map(|raw| {
            // The ex id follows the opcode byte.
            let packet = ExAutoSoulShot::decode(&mut PacketReader::new(&raw[3..])).unwrap();
            (packet.item_id, packet.shot_type)
        })
        .collect()
}

// ---- data --------------------------------------------------------------------------------------

const TEMPLATE: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

const SWORD_NG: i32 = 2;
const SOULSHOT_NG: i32 = 1835;
const SPIRITSHOT_NG: i32 = 2509;
const POTION: i32 = 1060;

/// A sword (no shared reuse group), shots, and a potion with shared reuse group 7.
const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<set name="soulshots" val="2" /><set name="spiritshots" val="2" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
<item id="1835" type="EtcItem" name="Soulshot: No Grade"><set name="default_action" val="SOULSHOT" />
<set name="is_stackable" val="true" /><set name="handler" val="SoulShots" /><skills><skill id="2039" level="1" /></skills></item>
<item id="2509" type="EtcItem" name="Spiritshot: No Grade"><set name="default_action" val="SPIRITSHOT" />
<set name="is_stackable" val="true" /><set name="handler" val="SpiritShot" /><skills><skill id="2061" level="1" /></skills></item>
<item id="1060" type="EtcItem" name="Healing Potion"><set name="shared_reuse_group" val="7" />
<set name="is_stackable" val="true" /></item>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            "<list></list>",
            &l2_data::skill::EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

const ME: ObjectId = ObjectId(0x2000_0001);
const SWORD_OBJ: ObjectId = ObjectId(0x3000_0001);
const SOULSHOT_OBJ: ObjectId = ObjectId(0x3000_0002);
const SPIRITSHOT_OBJ: ObjectId = ObjectId(0x3000_0003);
const POTION_OBJ: ObjectId = ObjectId(0x3000_0004);

fn item(object_id: ObjectId, item_id: i32, loc: &str, slot: usize, count: i64) -> ItemData {
    ItemData {
        object_id,
        item_id,
        count,
        enchant_level: 0,
        loc: loc.into(),
        loc_data: i32::try_from(slot).unwrap(),
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

fn items() -> Vec<ItemData> {
    vec![
        item(SWORD_OBJ, SWORD_NG, "PAPERDOLL", paperdoll::RHAND, 1),
        item(SOULSHOT_OBJ, SOULSHOT_NG, "INVENTORY", 0, 100),
        item(SPIRITSHOT_OBJ, SPIRITSHOT_NG, "INVENTORY", 1, 100),
        item(POTION_OBJ, POTION, "INVENTORY", 2, 10),
    ]
}

fn sc(slot: i32, page: i32, kind: i32, shortcut_id: i32, level: i32) -> ShortcutData {
    ShortcutData {
        slot,
        page,
        kind,
        shortcut_id,
        level,
    }
}

fn player_data(shortcuts: Vec<ShortcutData>) -> PlayerData {
    PlayerData {
        object_id: ME,
        account: "acc".into(),
        name: "Hero".into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: -71_338,
        y: 258_271,
        z: -3_104,
        heading: 0,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items: items(),
        skills: vec![],
        shortcuts,
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new(shortcuts: Vec<ShortcutData>) -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(20_261_020), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(shortcuts)),
            sink: Box::new(Sink(log.clone())),
            ip: None,
        });
        for _ in 0..20 {
            world.tick();
        }
        log.take();
        Self { world, log }
    }

    fn input(&mut self, input: ClientInput) -> Sent {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input,
        });
        self.log.take()
    }

    /// `RequestShortcutReg` with the character type 1 (the player).
    fn reg(&mut self, type_id: i32, slot: i32, id: i32, level: i32) -> Sent {
        self.reg_as(type_id, slot, id, level, 1)
    }

    fn reg_as(&mut self, type_id: i32, slot: i32, id: i32, level: i32, character_type: i32) -> Sent {
        self.input(ClientInput::ShortcutReg {
            type_id,
            slot,
            id,
            level,
            character_type,
        })
    }

    fn del(&mut self, slot: i32) -> Sent {
        self.input(ClientInput::ShortcutDel { slot })
    }

    fn bar(&self) -> Vec<ShortcutData> {
        self.world.player(ME).unwrap().data.shortcuts.clone()
    }
}

// ---- RequestShortcutReg ------------------------------------------------------------------------

/// A skill goes on page 1, slot 2 (wire slot 14); the client gets the skill form of the register
/// packet with the character type it sent.
#[test]
fn registering_a_skill() {
    let mut s = Scene::new(vec![]);
    let sent = s.reg(2, 14, 1204, 7);
    assert_eq!(names(&sent), ["SHORT_CUT_REGISTER"]);
    assert_eq!(
        registers(&sent),
        [ShortcutRegister::Skill {
            slot: 14,
            id: 1204,
            level: 7,
            character_type: 1
        }]
    );
    assert_eq!(s.bar(), [sc(2, 1, 2, 1204, 7)]);
}

/// The character type of the packet is the one of the request (2 is a pet), the row is the same.
#[test]
fn the_character_type_is_echoed() {
    let mut s = Scene::new(vec![]);
    let sent = s.reg_as(2, 0, 1204, 7, 2);
    assert_eq!(
        registers(&sent),
        [ShortcutRegister::Skill {
            slot: 0,
            id: 1204,
            level: 7,
            character_type: 2
        }]
    );
    assert_eq!(s.bar(), [sc(0, 0, 2, 1204, 7)]);
}

/// An item shortcut takes the shared reuse group of the item and needs the item in the inventory.
#[test]
fn registering_an_item() {
    let mut s = Scene::new(vec![]);
    let sent = s.reg(1, 3, POTION_OBJ.0, 0);
    assert_eq!(
        registers(&sent),
        [ShortcutRegister::Item {
            slot: 3,
            id: POTION_OBJ.0,
            character_type: 1,
            shared_reuse_group: 7
        }]
    );
    assert_eq!(s.bar(), [sc(3, 0, 1, POTION_OBJ.0, 0)]);
}

/// NOTE(java-quirk): a weapon gets the template's group, 0 (the list of `restoreMe` would say -1).
#[test]
fn an_item_without_a_group_shows_zero_in_the_register_packet() {
    let mut s = Scene::new(vec![]);
    let sent = s.reg(1, 0, SWORD_OBJ.0, 0);
    assert_eq!(
        registers(&sent),
        [ShortcutRegister::Item {
            slot: 0,
            id: SWORD_OBJ.0,
            character_type: 1,
            shared_reuse_group: 0
        }]
    );
}

/// NOTE(java-quirk): an item that is not there is not stored, but the client is still sent the
/// register packet (with group -1).
#[test]
fn an_unknown_item_is_refused_but_the_packet_is_sent() {
    let mut s = Scene::new(vec![]);
    let sent = s.reg(1, 4, 0x1234_5678, 0);
    assert_eq!(
        registers(&sent),
        [ShortcutRegister::Item {
            slot: 4,
            id: 0x1234_5678,
            character_type: 1,
            shared_reuse_group: -1
        }]
    );
    assert!(s.bar().is_empty());
}

#[test]
fn registering_actions_macros_and_recipes() {
    let mut s = Scene::new(vec![]);
    for (type_id, kind) in [
        (3, OtherShortcut::Action),
        (4, OtherShortcut::Macro),
        (5, OtherShortcut::Recipe),
        (6, OtherShortcut::Bookmark),
    ] {
        let slot = type_id; // 3..=6 on page 0
        let sent = s.reg_as(type_id, slot, 90 + type_id, 0, 2);
        assert_eq!(
            registers(&sent),
            [ShortcutRegister::Other {
                kind,
                slot,
                id: 90 + type_id,
                character_type: 2
            }]
        );
    }
    assert_eq!(
        s.bar(),
        [
            sc(3, 0, 3, 93, 0),
            sc(4, 0, 4, 94, 0),
            sc(5, 0, 5, 95, 0),
            sc(6, 0, 6, 96, 0)
        ]
    );
}

/// `ShortcutType.values()[(typeId < 1) || (typeId > 6) ? 0 : typeId]`: anything else is NONE. The
/// row is still stored with type 0 and the register packet has only the type and the slot.
#[test]
fn an_unknown_type_becomes_none() {
    let mut s = Scene::new(vec![]);
    for (n, type_id) in [0, 7, -1, 99].into_iter().enumerate() {
        let n = i32::try_from(n).unwrap();
        let sent = s.reg(type_id, n, 5, 6);
        assert_eq!(registers(&sent), [ShortcutRegister::Empty { slot: n }]);
        assert_eq!(s.bar().last(), Some(&sc(n, 0, 0, 5, 6)));
    }
    assert_eq!(s.bar().len(), 4);
}

/// The page is `slot / 12` and has to be 0..=10: wire slots up to 131 pass, 132 and up do not.
#[test]
fn the_page_has_to_be_between_0_and_10() {
    let mut s = Scene::new(vec![]);
    assert_eq!(registers(&s.reg(2, 131, 1, 1)).len(), 1);
    assert_eq!(s.bar(), [sc(11, 10, 2, 1, 1)]);
    // Page 11: no packet, no shortcut.
    assert!(s.reg(2, 132, 2, 1).is_empty());
    // A negative wire slot below -11 is a negative page.
    assert!(s.reg(2, -12, 3, 1).is_empty());
    assert!(s.reg(2, i32::MAX, 3, 1).is_empty());
    assert!(s.reg(2, i32::MIN, 3, 1).is_empty());
    assert_eq!(s.bar().len(), 1);
}

/// NOTE(java-quirk): `-5 % 12` is -5 and `-5 / 12` is 0 in Java, so a wire slot of -5 is slot -5
/// of page 0, and the packet shows it as -5 again.
#[test]
fn a_small_negative_slot_passes_like_in_java() {
    let mut s = Scene::new(vec![]);
    let sent = s.reg(2, -5, 1, 1);
    assert_eq!(
        registers(&sent),
        [ShortcutRegister::Skill {
            slot: -5,
            id: 1,
            level: 1,
            character_type: 1
        }]
    );
    assert_eq!(s.bar(), [sc(-5, 0, 2, 1, 1)]);
}

/// A new shortcut on a taken place replaces the old one, which keeps one row per place.
#[test]
fn a_shortcut_on_a_taken_place_replaces_it() {
    let mut s = Scene::new(vec![sc(0, 0, 2, 100, 1), sc(1, 0, 2, 101, 1)]);
    s.reg(2, 0, 200, 3);
    assert_eq!(s.bar(), [sc(0, 0, 2, 200, 3), sc(1, 0, 2, 101, 1)]);
    // The same skill on two places is fine.
    s.reg(2, 5, 101, 1);
    assert_eq!(s.bar().len(), 3);
}

/// The wire slot of the packet is `slot + page * 12` of the stored place.
#[test]
fn the_register_packet_slot_is_slot_plus_page_times_12() {
    let packet = shortcut_register_packet(&sc(11, 10, 2, 1, 1), 1, -1);
    assert_eq!(
        packet,
        ShortcutRegister::Skill {
            slot: 131,
            id: 1,
            level: 1,
            character_type: 1
        }
    );
    // A bad stored type reads as NONE.
    assert_eq!(
        shortcut_register_packet(&sc(1, 0, 42, 1, 1), 1, -1),
        ShortcutRegister::Empty { slot: 1 }
    );
}

// ---- RequestShortcutDel ------------------------------------------------------------------------

/// Java sends no confirmation of its own: `deleteShortcut` sends the whole list again.
#[test]
fn deleting_resends_the_list() {
    let mut s = Scene::new(vec![sc(0, 0, 2, 100, 1), sc(2, 1, 2, 101, 4), sc(3, 0, 3, 7, 0)]);
    let sent = s.del(14); // page 1, slot 2
    assert_eq!(names(&sent), ["SHORT_CUT_INIT"]);
    let init = inits(&sent).remove(0);
    assert_eq!(
        init.shortcuts,
        [
            ShortcutEntry::Skill {
                slot: 0,
                id: 100,
                level: 1
            },
            ShortcutEntry::Other {
                kind: OtherShortcut::Action,
                slot: 3,
                id: 7
            },
        ]
    );
    assert_eq!(s.bar(), [sc(0, 0, 2, 100, 1), sc(3, 0, 3, 7, 0)]);
}

/// An empty place and a page out of range do nothing, not even a packet.
#[test]
fn deleting_nothing_sends_nothing() {
    let mut s = Scene::new(vec![sc(0, 0, 2, 100, 1)]);
    assert!(s.del(5).is_empty());
    assert!(s.del(132).is_empty());
    assert!(s.del(-12).is_empty());
    assert!(s.del(-5).is_empty());
    assert_eq!(s.bar().len(), 1);
}

/// Register and delete again leave the bar as it was.
#[test]
fn register_then_delete() {
    let mut s = Scene::new(vec![]);
    s.reg(3, 7, 12, 0);
    assert_eq!(s.bar().len(), 1);
    let sent = s.del(7);
    assert!(inits(&sent)[0].shortcuts.is_empty());
    assert!(s.bar().is_empty());
}

/// `Shortcuts.deleteShortcut` for a shortcut to a shot: the automatic use of that shot ends (the
/// client is told before the list), and the shots that stay are announced again after the list.
#[test]
fn deleting_the_shortcut_of_a_shot_ends_its_automatic_use() {
    let mut s = Scene::new(vec![
        sc(0, 0, 1, SOULSHOT_OBJ.0, 0),
        sc(1, 0, 1, SPIRITSHOT_OBJ.0, 0),
    ]);
    s.input(ClientInput::AutoSoulShot {
        item_id: SOULSHOT_NG,
        shot_type: 1,
    });
    s.input(ClientInput::AutoSoulShot {
        item_id: SPIRITSHOT_NG,
        shot_type: 1,
    });
    s.log.take();
    let sent = s.del(0);
    assert_eq!(
        names(&sent),
        ["EX_AUTO_SOULSHOT", "SHORT_CUT_INIT", "EX_AUTO_SOULSHOT"],
        "{:?}",
        names(&sent)
    );
    assert_eq!(auto_shot_packets(&sent), [(SOULSHOT_NG, 0), (SPIRITSHOT_NG, 1)]);
    assert!(!s.world.has_auto_soul_shot(ME, SOULSHOT_NG));
    assert!(s.world.has_auto_soul_shot(ME, SPIRITSHOT_NG));
}

/// Deleting a plain item shortcut only resends the list (and the shots that are on).
#[test]
fn deleting_a_potion_shortcut_keeps_the_shots_on() {
    let mut s = Scene::new(vec![sc(0, 0, 1, POTION_OBJ.0, 0)]);
    s.input(ClientInput::AutoSoulShot {
        item_id: SOULSHOT_NG,
        shot_type: 1,
    });
    s.log.take();
    let sent = s.del(0);
    assert_eq!(names(&sent), ["SHORT_CUT_INIT", "EX_AUTO_SOULSHOT"]);
    assert_eq!(auto_shot_packets(&sent), [(SOULSHOT_NG, 1)]);
}

/// The list of a character screen restore shows the group of etc items only (`restoreMe`).
#[test]
fn the_list_shows_the_group_of_etc_items_and_minus_one_for_the_rest() {
    let mut s = Scene::new(vec![
        sc(0, 0, 1, POTION_OBJ.0, 0),
        sc(1, 0, 1, SWORD_OBJ.0, 0),
        sc(2, 0, 2, 5, 1),
    ]);
    let sent = s.del(2);
    assert_eq!(
        inits(&sent)[0].shortcuts,
        [
            ShortcutEntry::Item {
                slot: 0,
                id: POTION_OBJ.0,
                shared_reuse_group: 7
            },
            ShortcutEntry::Item {
                slot: 1,
                id: SWORD_OBJ.0,
                shared_reuse_group: -1
            },
        ]
    );
}
