#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Inventory and equipping (M4-INV-01): the paperdoll rules of Java's `Inventory`, the packets of
//! `UseItem` / `RequestUnEquipItem` / `RequestDestroyItem` / `RequestDropItem` /
//! `RequestItemList`, and the item rows that leave the world as `PersistCommand::ItemChanges`.
//!
//! The items are real item XML (the loader of the datapack), so body parts, grades, armor sets
//! and skills behave as in the server. Expected numbers come from the Java code named in each
//! test.

use std::sync::{Arc, Mutex};

use l2_data::{ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::enterworld::EtcStatusUpdate;
use l2_protocol_game::server::{
    ExQuestItemList, InventoryUpdate, ItemList, ShortcutInit, StatusUpdate, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(i32, Vec<u8>),
    Persist(PersistCommand),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }

    fn packets(&self) -> Vec<Vec<u8>> {
        self.take()
            .into_iter()
            .filter_map(|o| match o {
                Out::Packet(_, bytes) => Some(bytes),
                Out::Persist(_) => None,
            })
            .collect()
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push(Out::Packet(self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct RecOutbox(Log);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

// ---- the datapack -------------------------------------------------------------------------------

/// Class 0: 40 STR, 43 CON, 100 HP at level 1.
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

const ITEMS: &str = r#"<list>
<item id="1" type="Weapon" name="S Bow"><set name="crystal_type" val="S" /><set name="weapon_type" val="BOW" /><set name="bodypart" val="lrhand" /><set name="weight" val="100" />
<stats><stat type="pAtk">100</stat><stat type="pAtkSpd">293</stat></stats></item>
<item id="2" type="Weapon" name="C Sword"><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat><stat type="pAtkSpd">379</stat></stats></item>
<item id="3" type="Armor" name="D Breastplate"><set name="crystal_type" val="D" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="weight" val="10" />
<stats><stat type="pDef">40</stat></stats></item>
<item id="4" type="Armor" name="D Gaiters"><set name="crystal_type" val="D" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="legs" /><set name="weight" val="10" />
<stats><stat type="pDef">25</stat></stats></item>
<item id="5" type="Armor" name="Shield"><set name="crystal_type" val="D" /><set name="bodypart" val="lhand" /><set name="weight" val="10" />
<stats><stat type="sDef">50</stat></stats></item>
<item id="6" type="Armor" name="Ring"><set name="bodypart" val="rfinger;lfinger" /><set name="weight" val="1" />
<stats><stat type="mDef">10</stat></stats></item>
<item id="7" type="Armor" name="Robe"><set name="armor_type" val="MAGIC" /><set name="bodypart" val="fullarmor" /><set name="weight" val="10" />
<stats><stat type="pDef">30</stat></stats></item>
<item id="8" type="Armor" name="Earring"><set name="bodypart" val="rear;lear" /><set name="weight" val="1" />
<stats><stat type="mDef">9</stat></stats></item>
<item id="9" type="EtcItem" name="S Arrow"><set name="etcitem_type" val="ARROW" /><set name="bodypart" val="lhand" /><set name="crystal_type" val="S" /><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="10" type="Weapon" name="Other C Sword"><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">60</stat></stats></item>
<item id="11" type="Armor" name="Cloak"><set name="bodypart" val="back" /><set name="weight" val="1" />
<conditions msgId="113"><using kind="HEAVY" /></conditions>
<stats><stat type="pDef">3</stat></stats></item>
<item id="12" type="Armor" name="Bracelet"><set name="bodypart" val="rbracelet" /><set name="weight" val="1" /></item>
<item id="13" type="EtcItem" name="Talisman"><set name="bodypart" val="deco1" /><set name="weight" val="1" /></item>
<item id="14" type="Weapon" name="Greatsword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="lrhand" /><set name="weight" val="60" /></item>
<item id="15" type="Armor" name="Formal Wear"><set name="bodypart" val="alldress" /><set name="weight" val="5" /></item>
<item id="16" type="Weapon" name="Rapier"><set name="weapon_type" val="RAPIER" /><set name="bodypart" val="rhand" /><set name="weight" val="5" /></item>
<item id="17" type="Weapon" name="Skilled Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="5" />
<skills><skill id="3007" level="1" /></skills></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="20" type="EtcItem" name="Anvil"><set name="weight" val="1000" /><set name="is_stackable" val="true" /></item>
<item id="21" type="EtcItem" name="Quest thing"><set name="is_questitem" val="true" /><set name="is_stackable" val="true" /></item>
<item id="22" type="EtcItem" name="Bound"><set name="is_dropable" val="false" /><set name="is_destroyable" val="false" /><set name="weight" val="1" /></item>
<item id="23" type="EtcItem" name="Pile"><set name="is_stackable" val="true" /><set name="weight" val="2" /></item>
<item id="25" type="EtcItem" name="Talisman 2"><set name="bodypart" val="deco1" /><set name="weight" val="1" /></item>
<item id="24" type="Weapon" name="Force"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="isForceEquip" val="true" /></item>
</list>"#;

const SKILLS: &str = r##"<list>
<skill id="3006" levels="1" name="Set"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="mDef" val="7"/></effect></effects></skill>
<skill id="3007" levels="1" name="Sword skill"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="critRate" val="11"/></effect></effects></skill>
<skill id="6209" levels="4" name="Weapon grade penalty"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><mul stat="pAtk" val="0.9"/></effect></effects></skill>
<skill id="6213" levels="4" name="Armor grade penalty"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><mul stat="pDef" val="0.9"/></effect></effects></skill>
<skill id="4270" levels="4" name="Weight penalty"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><mul stat="runSpd" val="0.9"/></effect></effects></skill>
<skill id="3300" levels="1" name="Talisman slots"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="TalismanSlot"><slots>2</slots></effect></effects></skill>
</list>"##;

struct Data {
    items: Arc<ItemTemplates>,
    stats: StatData,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap(),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: t.shared_reuse_group,
            weapon: t.is_weapon(),
        })
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.stats
            .skills
            .iter()
            .any(|s| s.id == skill_id && s.is_passive())
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
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
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

fn data() -> Data {
    let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
    let stats = StatData {
        items: Arc::clone(&items),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(
            ArmorSets::from_xml(
                r#"<list><set id="1"><chest id="3" /><legs id="4" /><skill id="3006" level="1" /><str val="2" /><con val="-1" /></set></list>"#,
            )
            .unwrap(),
        ),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

// ---- helpers ------------------------------------------------------------------------------------

const ME: ObjectId = ObjectId(1);
const SESSION: SessionId = SessionId(1);

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

fn inv(object_id: i32, item_id: i32) -> ItemData {
    item(object_id, item_id, 1, "INVENTORY", 0)
}

fn worn(object_id: i32, item_id: i32, slot: usize) -> ItemData {
    item(object_id, item_id, 1, "PAPERDOLL", i32::try_from(slot).unwrap())
}

fn player(items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: ME,
        account: "acc".into(),
        name: "Me".into(),
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
        x: 1000,
        y: 1000,
        z: 0,
        heading: 0,
        cur_hp: 50.0,
        max_hp: 100.0,
        cur_mp: 30.0,
        max_mp: 50.0,
        cur_cp: 0.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

/// A world with one player in it; the log is empty when this returns.
fn world_with(p: PlayerData) -> (World, Log) {
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
    world.set_static_data(Box::new(data()));
    world.handle(WorldCommand::EnterWorld {
        session: SESSION,
        player: Box::new(p),
        sink: Box::new(Sink(1, log.clone())),
        ip: None,
    });
    log.take();
    (world, log)
}

fn input(world: &mut World, input: ClientInput) {
    world.handle(WorldCommand::Input {
        session: SESSION,
        input,
    });
}

fn use_item(world: &mut World, object_id: i32) {
    input(
        world,
        ClientInput::UseItem {
            object_id,
            ctrl_pressed: false,
        },
    );
}

/// Runs the world for `ms` milliseconds (50 ms ticks), so the delayed sends go out.
fn advance(world: &mut World, ms: u64) {
    for _ in 0..(ms / 50) {
        world.tick();
    }
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

fn find<'a>(packets: &'a [Vec<u8>], name: &str) -> Vec<&'a Vec<u8>> {
    packets.iter().filter(|p| name_of(p) == name).collect()
}

/// The ids of the system messages, without decoding them (a message can declare more
/// parameters than the sender added).
fn message_ids(packets: &[Vec<u8>]) -> Vec<i32> {
    find(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| i32::from_le_bytes([p[1], p[2], p[3], p[4]]))
        .collect()
}

fn system_messages(packets: &[Vec<u8>]) -> Vec<SystemMessage> {
    find(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p))
        .collect()
}

/// The slot of each object the player wears, `(slot, object id)`, by slot.
fn worn_slots(world: &World) -> Vec<(i32, i32)> {
    let mut slots: Vec<(i32, i32)> = world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.loc == "PAPERDOLL")
        .map(|i| (i.loc_data, i.object_id.0))
        .collect();
    slots.sort_unstable();
    slots
}

fn slot_of(world: &World, object_id: i32) -> (String, i32) {
    let i = world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.object_id.0 == object_id)
        .unwrap();
    (i.loc.clone(), i.loc_data)
}

/// `(object id, loc, loc_data)` of the rows written by one `ItemChanges`.
type Rows = Vec<(i32, String, i32)>;

fn item_changes(out: &[Out]) -> Vec<(Rows, Vec<i32>)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemChanges {
                owner,
                upserts,
                removed,
            }) => {
                assert_eq!(*owner, ME);
                Some((
                    upserts
                        .iter()
                        .map(|i| (i.object_id.0, i.loc.clone(), i.loc_data))
                        .collect(),
                    removed.iter().map(|o| o.0).collect(),
                ))
            }
            _ => None,
        })
        .collect()
}

// ---- weapons and the shield ----------------------------------------------------------------------

#[test]
fn equipping_a_weapon_sends_java_s_packets_and_writes_the_row() {
    let (mut world, log) = world_with(player(vec![inv(100, 2)]));
    use_item(&mut world, 100);
    let out = log.take();
    let packets: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(_, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    // `setPaperdollItem` -> `StatsListener.addStatFuncs` -> `broadcastModifiedStats`: UserInfo
    // (`updateUserInfo`) and the attack speed in a StatusUpdate; then "You have equipped your
    // C Sword"; `refreshExpertisePenalty` (a C weapon without Expertise) -> EtcStatusUpdate;
    // `broadcastUserInfo`.
    assert_eq!(
        names(&packets),
        [
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "STATUS_UPDATE",
            "SYSTEM_MESSAGE",
            "ETC_STATUS_UPDATE",
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
        ]
    );
    let su: StatusUpdate = decode(&packets[2]);
    assert_eq!(su.object_id, 1);
    assert_eq!(su.attributes.len(), 1);
    let sm = &system_messages(&packets)[0];
    assert_eq!(sm.id, SystemMessageId::YouHaveEquippedYourS1);
    assert_eq!(sm.params, [SystemMessageParam::ItemName(2)]);
    let etc: EtcStatusUpdate = decode(&packets[4]);
    // `CrystalType.C.getLevel()` is 2 and the player has no Expertise.
    assert_eq!(etc.expertise_weapon_penalty, 2);
    // The item is worn, and its row was written once, at once (Item.updateDatabase).
    assert_eq!(worn_slots(&world), [(5, 100)]);
    assert_eq!(
        item_changes(&out),
        [(vec![(100, "PAPERDOLL".to_string(), 5)], vec![])]
    );

    // 100 ms later the InventoryUpdate goes out with the item marked as modified.
    advance(&mut world, 100);
    let later = log.packets();
    assert_eq!(names(&later), ["INVENTORY_UPDATE"]);
    let iu: InventoryUpdate = decode(&later[0]);
    assert_eq!(iu.updates.len(), 1);
    assert_eq!(iu.updates[0].change, InventoryUpdate::MODIFIED);
    assert_eq!(iu.updates[0].item.object_id, 100);
    assert_eq!(iu.updates[0].item.equipped, 1);
    assert_eq!(iu.updates[0].item.location, 5);
}

#[test]
fn using_a_worn_item_takes_it_off() {
    let (mut world, log) = world_with(player(vec![worn(100, 2, paperdoll::RHAND)]));
    use_item(&mut world, 100);
    let out = log.take();
    let packets: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(_, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    // The "disarmed" message comes first, then the stat functions go (UserInfo, StatusUpdate),
    // the expertise penalty ends (`removeSkill(.., true)` stops the passive with REMOVED: "The
    // effect of $s1 has been removed.", then EtcStatusUpdate), `broadcastUserInfo`.
    assert_eq!(
        names(&packets),
        [
            "SYSTEM_MESSAGE",
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "STATUS_UPDATE",
            "SYSTEM_MESSAGE",
            "ETC_STATUS_UPDATE",
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
        ]
    );
    assert_eq!(
        system_messages(&packets)[0].id,
        SystemMessageId::S1HasBeenDisarmed
    );
    assert_eq!(worn_slots(&world), []);
    assert_eq!(slot_of(&world, 100), ("INVENTORY".to_string(), 0));
    assert_eq!(
        item_changes(&out),
        [(vec![(100, "INVENTORY".to_string(), 0)], vec![])]
    );
}

#[test]
fn an_enchanted_item_has_its_own_messages() {
    let mut sword = worn(100, 2, paperdoll::RHAND);
    sword.enchant_level = 4;
    let (mut world, log) = world_with(player(vec![sword, {
        let mut other = inv(101, 10);
        other.enchant_level = 7;
        other
    }]));
    use_item(&mut world, 100);
    let messages = system_messages(&log.packets());
    assert_eq!(messages[0].id, SystemMessageId::TheEquipmentS1S2HasBeenRemoved);
    assert_eq!(
        messages[0].params,
        [SystemMessageParam::IntNumber(4), SystemMessageParam::ItemName(2)]
    );
    use_item(&mut world, 101);
    let messages = system_messages(&log.packets());
    assert_eq!(messages[0].id, SystemMessageId::EquippedS1S2);
    assert_eq!(
        messages[0].params,
        [SystemMessageParam::IntNumber(7), SystemMessageParam::ItemName(10)]
    );
}

#[test]
fn a_new_weapon_replaces_the_old_one_and_both_rows_are_written_in_order() {
    let (mut world, log) = world_with(player(vec![worn(100, 2, paperdoll::RHAND), inv(101, 10)]));
    use_item(&mut world, 101);
    let out = log.take();
    assert_eq!(worn_slots(&world), [(5, 101)]);
    // `setPaperdollItem`: the old item goes back to INVENTORY (and is written) before the new one
    // goes into the slot (and is written).
    assert_eq!(
        item_changes(&out),
        [
            (vec![(100, "INVENTORY".to_string(), 0)], vec![]),
            (vec![(101, "PAPERDOLL".to_string(), 5)], vec![]),
        ]
    );
    // Both are in the delayed InventoryUpdate.
    advance(&mut world, 100);
    let iu: InventoryUpdate = decode(&log.packets()[0]);
    let mut ids: Vec<i32> = iu.updates.iter().map(|u| u.item.object_id).collect();
    ids.sort_unstable();
    assert_eq!(ids, [100, 101]);
}

#[test]
fn a_two_hander_takes_the_shield_off_and_a_shield_takes_the_two_hander_off() {
    let (mut world, log) = world_with(player(vec![
        worn(100, 5, paperdoll::LHAND),
        inv(101, 14),
        inv(102, 2),
    ]));
    // Inventory.equipItem LR_HAND: "setPaperdollItem(LHAND, null); setPaperdollItem(RHAND, item)".
    use_item(&mut world, 101);
    assert_eq!(worn_slots(&world), [(5, 101)]);
    assert_eq!(slot_of(&world, 100), ("INVENTORY".to_string(), 0));
    // L_HAND with a two-handed weapon in the right hand: the weapon comes off first.
    use_item(&mut world, 100);
    assert_eq!(worn_slots(&world), [(7, 100)]);
    assert_eq!(slot_of(&world, 101), ("INVENTORY".to_string(), 0));
    // A one-hander next to a shield: both stay.
    use_item(&mut world, 102);
    assert_eq!(worn_slots(&world), [(5, 102), (7, 100)]);
    log.take();
}

#[test]
fn a_bow_takes_the_arrows_and_the_arrows_go_with_the_bow() {
    let arrows = item(200, 9, 500, "INVENTORY", 0);
    let (mut world, log) = world_with(player(vec![inv(100, 1), arrows]));
    use_item(&mut world, 100);
    let out = log.take();
    // BowCrossRodListener: arrows of the bow's grade go into the left hand.
    assert_eq!(worn_slots(&world), [(5, 100), (7, 200)]);
    // The arrow is written first (its own setPaperdollItem runs inside the bow's).
    assert_eq!(
        item_changes(&out),
        [
            (vec![(200, "PAPERDOLL".to_string(), 7)], vec![]),
            (vec![(100, "PAPERDOLL".to_string(), 5)], vec![]),
        ]
    );
    advance(&mut world, 100);
    log.take();

    // Taking the bow off takes the arrows off. The ChangeRecorder saw the arrows first, which
    // is why "$s1 has been disarmed" of RequestUnEquipItem names the arrows.
    input(&mut world, ClientInput::UnEquipItem { paperdoll_slot: 5 });
    let out = log.take();
    assert_eq!(worn_slots(&world), []);
    let packets: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(_, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    // The grade penalty of the bow ends first (inside the unequip): "The effect of $s1 has been
    // removed." for the passive penalty skill, then the message of RequestUnEquipItem.
    let messages = system_messages(&packets);
    assert_eq!(messages[0].id, SystemMessageId::TheEffectOfS1HasBeenRemoved);
    assert_eq!(
        messages[0].params,
        [SystemMessageParam::SkillName { id: 6209, level: 4 }]
    );
    let message = &messages[1];
    assert_eq!(message.id, SystemMessageId::S1HasBeenDisarmed);
    assert_eq!(message.params, [SystemMessageParam::ItemName(9)]);
    assert_eq!(
        item_changes(&out),
        [
            (vec![(200, "INVENTORY".to_string(), 0)], vec![]),
            (vec![(100, "INVENTORY".to_string(), 0)], vec![]),
        ]
    );
    advance(&mut world, 100);
    let iu: InventoryUpdate = decode(&log.packets()[0]);
    assert_eq!(iu.updates.len(), 2);
}

#[test]
fn arrows_cannot_be_taken_off_alone() {
    let (mut world, log) = world_with(player(vec![worn(100, 1, paperdoll::RHAND), {
        let mut arrows = item(200, 9, 500, "PAPERDOLL", 7);
        arrows.count = 500;
        arrows
    }]));
    input(&mut world, ClientInput::UnEquipItem { paperdoll_slot: 7 });
    // `_slot == L_HAND && template instanceof EtcItem`: nothing happens, not even a message.
    assert!(log.take().is_empty());
    assert_eq!(worn_slots(&world), [(5, 100), (7, 200)]);
}

// ---- armor -----------------------------------------------------------------------------------------

#[test]
fn full_armor_takes_the_legs_and_legs_take_the_full_armor() {
    let (mut world, _log) = world_with(player(vec![
        worn(100, 4, paperdoll::LEGS),
        inv(101, 7),
        inv(102, 4),
        inv(103, 3),
    ]));
    use_item(&mut world, 101);
    assert_eq!(worn_slots(&world), [(6, 101)]);
    // LEGS with a full armor in the chest slot: the chest slot is emptied.
    use_item(&mut world, 102);
    assert_eq!(worn_slots(&world), [(11, 102)]);
    // A chest piece does not touch the legs.
    use_item(&mut world, 103);
    assert_eq!(worn_slots(&world), [(6, 103), (11, 102)]);
}

#[test]
fn formal_wear_takes_everything_and_blocks_the_slots_it_covers() {
    let (mut world, _log) = world_with(player(vec![
        worn(100, 2, paperdoll::RHAND),
        worn(101, 4, paperdoll::LEGS),
        worn(102, 5, paperdoll::LHAND),
        inv(103, 15),
        inv(104, 10),
        inv(105, 6),
    ]));
    use_item(&mut world, 103);
    // ALLDRESS: legs, left and right hand, head, feet and gloves come off; it goes into the chest.
    assert_eq!(worn_slots(&world), [(6, 103)]);
    // With formal wear on, a weapon cannot be equipped ("Only chest target can pass this").
    use_item(&mut world, 104);
    assert_eq!(worn_slots(&world), [(6, 103)]);
    // But jewellery can.
    use_item(&mut world, 105);
    assert_eq!(worn_slots(&world), [(6, 103), (14, 105)]);
}

#[test]
fn rings_and_earrings_fill_the_free_slot_and_then_replace_the_left_one() {
    let (mut world, _log) = world_with(player(vec![
        inv(100, 6),
        inv(101, 6),
        inv(102, 6),
        inv(103, 8),
        inv(104, 8),
        inv(105, 8),
    ]));
    for object_id in [100, 101, 102] {
        use_item(&mut world, object_id);
    }
    // LFINGER first, then RFINGER, then the left one is replaced.
    assert_eq!(worn_slots(&world), [(13, 101), (14, 102)]);
    assert_eq!(slot_of(&world, 100), ("INVENTORY".to_string(), 0));
    for object_id in [103, 104, 105] {
        use_item(&mut world, object_id);
    }
    // LEAR, REAR, then LEAR again.
    assert_eq!(worn_slots(&world), [(8, 104), (9, 105), (13, 101), (14, 102)]);
}

#[test]
fn an_armor_set_gives_its_bonus_and_skill_only_while_complete() {
    let (mut world, log) = world_with(player(vec![inv(100, 3), inv(101, 4)]));
    let base_str = world.stats(ME).unwrap().str();
    use_item(&mut world, 100);
    // Chest alone: no set bonus (FuncArmorSet needs every part).
    assert_eq!(world.stats(ME).unwrap().str(), base_str);
    assert!(
        !world
            .player(ME)
            .unwrap()
            .core
            .skills
            .iter()
            .any(|(id, _)| *id == 3006)
    );
    use_item(&mut world, 101);
    // Complete: STR +2, CON -1 and the set skill.
    assert_eq!(world.stats(ME).unwrap().str(), base_str + 2);
    assert!(world.player(ME).unwrap().core.skills.contains(&(3006, 1)));
    // The skill list goes out 300 ms after the change (one for both changes).
    log.take();
    advance(&mut world, 400);
    let packets = log.packets();
    let lists = find(&packets, "SKILL_LIST");
    assert_eq!(lists.len(), 1);
    let list: l2_protocol_game::server::enterworld::SkillList = decode(lists[0]);
    assert!(list.skills.iter().any(|s| s.id == 3006 && s.passive));

    // Taking the legs off ends it.
    use_item(&mut world, 101);
    assert_eq!(world.stats(ME).unwrap().str(), base_str);
    assert!(
        !world
            .player(ME)
            .unwrap()
            .core
            .skills
            .iter()
            .any(|(id, _)| *id == 3006)
    );
}

#[test]
fn taking_off_the_chest_of_a_set_checks_the_conditions_of_the_other_pieces() {
    // The cloak needs heavy armor (`using kind=HEAVY`). Without the chest it does not hold, and
    // `ArmorSetListener.notifyUnequiped` -> `checkItemRestriction` takes the cloak off.
    let (mut world, log) = world_with(player(vec![
        worn(100, 3, paperdoll::CHEST),
        worn(102, 11, paperdoll::CLOAK),
    ]));
    // (The cloak's condition holds at enter world: heavy armor is worn.)
    assert_eq!(worn_slots(&world), [(6, 100), (23, 102)]);
    use_item(&mut world, 100);
    assert_eq!(worn_slots(&world), []);
    let out = log.take();
    let packets: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(_, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    assert!(
        system_messages(&packets)
            .iter()
            .any(|m| m.id == SystemMessageId::YourCloakHasBeenUnequippedBecauseYourArmorSetIsNoLongerComplete)
    );
    // The rows: chest to INVENTORY, cloak to INVENTORY (its own setPaperdollItem runs inside the
    // chest's listener, before the chest's row is written).
    assert_eq!(
        item_changes(&out),
        [
            (vec![(102, "INVENTORY".to_string(), 0)], vec![]),
            (vec![(100, "INVENTORY".to_string(), 0)], vec![]),
        ]
    );
}

// ---- items with skills, talismans, bracelets ---------------------------------------------------

#[test]
fn an_item_skill_comes_and_goes_with_the_item() {
    let (mut world, log) = world_with(player(vec![inv(100, 17)]));
    use_item(&mut world, 100);
    assert!(world.player(ME).unwrap().core.skills.contains(&(3007, 1)));
    log.take();
    advance(&mut world, 400);
    assert_eq!(find(&log.packets(), "SKILL_LIST").len(), 1);
    use_item(&mut world, 100);
    assert!(
        !world
            .player(ME)
            .unwrap()
            .core
            .skills
            .iter()
            .any(|(id, _)| *id == 3007)
    );
    advance(&mut world, 400);
    assert_eq!(find(&log.packets(), "SKILL_LIST").len(), 1);
}

#[test]
fn talismans_need_slots_and_leave_with_the_right_bracelet() {
    let mut p = player(vec![
        inv(100, 13),
        inv(101, 25),
        inv(102, 13),
        worn(103, 12, paperdoll::RBRACELET),
    ]);
    p.skills = vec![SkillData {
        skill_id: 3300,
        skill_level: 1,
    }];
    let (mut world, _log) = world_with(p);
    // Two talisman slots: the third talisman of the same kind overwrites the first.
    use_item(&mut world, 100);
    use_item(&mut world, 101);
    assert_eq!(worn_slots(&world), [(16, 103), (17, 100), (18, 101)]);
    use_item(&mut world, 102);
    // "Find same (or incompatible) talisman type" -> overwrite DECO1.
    assert_eq!(worn_slots(&world), [(16, 103), (17, 102), (18, 101)]);
    // BraceletListener: the right bracelet off takes every talisman off.
    use_item(&mut world, 103);
    assert_eq!(worn_slots(&world), []);
}

#[test]
fn a_talisman_without_slots_cannot_be_used() {
    let (mut world, log) = world_with(player(vec![inv(100, 13)]));
    use_item(&mut world, 100);
    assert_eq!(worn_slots(&world), []);
    let messages = system_messages(&log.packets());
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].id,
        SystemMessageId::YouDoNotMeetTheRequiredConditionToEquipThatItem
    );
}

// ---- who may equip what ---------------------------------------------------------------------------

#[test]
fn races_cannot_wear_each_other_s_weapons() {
    let (mut world, log) = world_with(player(vec![inv(100, 16)]));
    use_item(&mut world, 100);
    // A human with a rapier (Kamael only).
    assert_eq!(worn_slots(&world), []);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::YouDoNotMeetTheRequiredConditionToEquipThatItem
    );
    // A GM may.
    let mut gm = player(vec![inv(100, 16)]);
    gm.access_level = 100;
    let (mut world, _log) = world_with(gm);
    use_item(&mut world, 100);
    assert_eq!(worn_slots(&world), [(5, 100)]);
}

#[test]
fn a_quest_item_cannot_be_used_and_the_condition_message_is_sent() {
    let (mut world, log) = world_with(player(vec![inv(100, 21), inv(101, 11)]));
    use_item(&mut world, 100);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::YouCannotUseQuestItems
    );
    // The cloak needs heavy armor and none is worn: the item's own message (msgId 113).
    use_item(&mut world, 101);
    let packets = log.packets();
    let ids = message_ids(&packets);
    assert_eq!(ids, [113]);
    assert_eq!(worn_slots(&world), []);
}

#[test]
fn a_dead_player_and_a_missing_item_do_nothing() {
    let mut dead = player(vec![inv(100, 2)]);
    dead.cur_hp = 0.0;
    let (mut world, log) = world_with(dead);
    use_item(&mut world, 100);
    assert!(log.take().is_empty());
    let (mut world, log) = world_with(player(vec![]));
    use_item(&mut world, 12345);
    assert!(log.take().is_empty());
}

// ---- RequestUnEquipItem -----------------------------------------------------------------------------

#[test]
fn unequip_by_slot_reports_the_item_and_broadcasts() {
    let (mut world, log) = world_with(player(vec![worn(100, 3, paperdoll::CHEST)]));
    input(&mut world, ClientInput::UnEquipItem { paperdoll_slot: 6 });
    let out = log.take();
    let packets: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(_, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    // StatsListener (UserInfo), `refreshExpertisePenalty` (a D armor without Expertise had a
    // penalty: its passive ends with "The effect of $s1 has been removed."),
    // `broadcastUserInfo`, then the message.
    assert_eq!(
        names(&packets),
        [
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "SYSTEM_MESSAGE",
            "ETC_STATUS_UPDATE",
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "SYSTEM_MESSAGE"
        ]
    );
    assert_eq!(worn_slots(&world), []);
    // An empty slot, a slot that is no body part and a negative slot do nothing.
    for slot in [6, 25, -1, 99] {
        input(&mut world, ClientInput::UnEquipItem { paperdoll_slot: slot });
    }
    assert!(log.take().is_empty());
}

#[test]
fn a_force_equip_weapon_stays_on_for_players() {
    let (mut world, log) = world_with(player(vec![worn(100, 24, paperdoll::RHAND)]));
    input(&mut world, ClientInput::UnEquipItem { paperdoll_slot: 5 });
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::ThatItemCannotBeTakenOff
    );
    assert_eq!(worn_slots(&world), [(5, 100)]);
}

#[test]
fn a_talisman_request_always_takes_the_first_talisman_slot() {
    let mut p = player(vec![
        worn(100, 13, paperdoll::DECO1),
        worn(101, 13, paperdoll::DECO3),
    ]);
    p.skills = vec![SkillData {
        skill_id: 3300,
        skill_level: 1,
    }];
    let (mut world, _log) = world_with(p);
    // NOTE(java-quirk): `BodyPart.fromPaperdollSlot(DECO3)` is `DECO`, whose slot is DECO1.
    input(&mut world, ClientInput::UnEquipItem { paperdoll_slot: 19 });
    assert_eq!(worn_slots(&world), [(19, 101)]);
}

// ---- penalties --------------------------------------------------------------------------------------

#[test]
fn the_grade_penalty_follows_what_is_worn() {
    let (mut world, log) = world_with(player(vec![inv(100, 1)]));
    use_item(&mut world, 100);
    // An S bow without Expertise: 5 - 0 - 0 clamped to 4. The penalty skill is added.
    let player = world.player(ME).unwrap();
    assert_eq!(player.expertise_weapon_penalty, 4);
    assert!(player.core.skills.contains(&(6209, 4)));
    let packets = log.packets();
    let etc: EtcStatusUpdate = decode(find(&packets, "ETC_STATUS_UPDATE")[0]);
    assert_eq!(etc.expertise_weapon_penalty, 4);
    use_item(&mut world, 100);
    let player = world.player(ME).unwrap();
    assert_eq!(player.expertise_weapon_penalty, 0);
    assert!(!player.core.skills.iter().any(|(id, _)| *id == 6209));
}

#[test]
fn weight_penalty_follows_the_load() {
    // 43 CON: the load limit is what the stat engine says; an anvil of 70% of it puts the player
    // in the first penalty levels.
    let probe = world_with(player(vec![])).0;
    let max_load = probe.stats(ME).unwrap().max_load();
    assert!(max_load > 0);
    let anvils = i64::from(max_load) * 7 / 10 / 1000 + 1;
    let (mut world, log) = world_with(player(vec![item(100, 20, anvils, "INVENTORY", 0)]));
    let before = world.player(ME).unwrap().weight_penalty;
    assert!(
        before > 0,
        "load {} of {max_load}",
        world.stats(ME).unwrap().current_load()
    );
    assert!(
        world
            .player(ME)
            .unwrap()
            .core
            .skills
            .iter()
            .any(|(id, _)| *id == 4270)
    );

    // Destroying them: the penalty ends -> UserInfo (broadcastUserInfo) and EtcStatusUpdate,
    // sent from inside `destroyItem`, before the InventoryUpdate.
    input(
        &mut world,
        ClientInput::DestroyItem {
            object_id: 100,
            count: anvils,
        },
    );
    let packets = log.packets();
    assert_eq!(world.player(ME).unwrap().weight_penalty, 0);
    assert!(
        !world
            .player(ME)
            .unwrap()
            .core
            .skills
            .iter()
            .any(|(id, _)| *id == 4270)
    );
    let n = names(&packets);
    let etc = n.iter().position(|p| p == "ETC_STATUS_UPDATE").unwrap();
    let iu = n.iter().position(|p| p == "INVENTORY_UPDATE").unwrap();
    assert!(etc < iu, "{n:?}");
}

// ---- destroying -------------------------------------------------------------------------------------

#[test]
fn destroying_a_stack_partly_writes_nothing_until_the_next_store() {
    let (mut world, log) = world_with(player(vec![item(100, 23, 10, "INVENTORY", 0)]));
    input(
        &mut world,
        ClientInput::DestroyItem {
            object_id: 100,
            count: 4,
        },
    );
    let out = log.take();
    // Java: `changeCount` only marks the item dirty; `updateDatabase()` is not called.
    assert!(item_changes(&out).is_empty());
    let packets: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(_, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    // InventoryUpdate (now), StatusUpdate CUR_LOAD, the message with the count.
    assert_eq!(
        names(&packets),
        ["INVENTORY_UPDATE", "STATUS_UPDATE", "SYSTEM_MESSAGE"]
    );
    let iu: InventoryUpdate = decode(&packets[0]);
    assert_eq!(iu.updates[0].change, InventoryUpdate::MODIFIED);
    assert_eq!(iu.updates[0].item.count, 6);
    let su: StatusUpdate = decode(&packets[1]);
    // 6 piles of weight 2 left.
    assert_eq!(
        su.attributes,
        [(l2_protocol_game::server::visibility::status::CUR_LOAD, 12)]
    );
    let sm = &system_messages(&packets)[0];
    assert_eq!(sm.id, SystemMessageId::S2S1HasDisappeared);
    assert_eq!(
        sm.params,
        [
            SystemMessageParam::ItemName(23),
            SystemMessageParam::LongNumber(4)
        ]
    );
    // The store still sees 6 (and would write the difference).
    assert_eq!(world.player(ME).unwrap().data.items[0].count, 6);
}

#[test]
fn destroying_a_whole_item_deletes_the_row_at_once() {
    let (mut world, log) = world_with(player(vec![inv(100, 2), inv(101, 3)]));
    input(
        &mut world,
        ClientInput::DestroyItem {
            object_id: 100,
            count: 1,
        },
    );
    let out = log.take();
    assert_eq!(item_changes(&out), [(vec![], vec![100])]);
    assert!(
        world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .all(|i| i.object_id.0 != 100)
    );
    let packets: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(_, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    let iu: InventoryUpdate = decode(find(&packets, "INVENTORY_UPDATE")[0]);
    assert_eq!(iu.updates[0].change, InventoryUpdate::REMOVED);
    assert_eq!(iu.updates[0].item.count, 0);
    let sm = &system_messages(&packets)[0];
    assert_eq!(sm.id, SystemMessageId::S1HasDisappeared);
}

#[test]
fn destroying_a_worn_item_takes_it_off_first_and_deletes_it() {
    let (mut world, log) = world_with(player(vec![worn(100, 2, paperdoll::RHAND)]));
    input(
        &mut world,
        ClientInput::DestroyItem {
            object_id: 100,
            count: 1,
        },
    );
    let out = log.take();
    // UPDATE to INVENTORY (the unequip), then DELETE.
    assert_eq!(
        item_changes(&out),
        [
            (vec![(100, "INVENTORY".to_string(), 0)], vec![]),
            (vec![], vec![100]),
        ]
    );
    let packets: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(_, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    let messages = system_messages(&packets);
    assert_eq!(messages[0].id, SystemMessageId::S1HasBeenDisarmed);
    assert_eq!(messages.last().unwrap().id, SystemMessageId::S1HasDisappeared);
    // The unequip InventoryUpdate goes out at once, before the destruction one.
    let n = names(&packets);
    let updates: Vec<usize> = n
        .iter()
        .enumerate()
        .filter(|(_, p)| *p == "INVENTORY_UPDATE")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(updates.len(), 2);
    let first: InventoryUpdate = decode(&packets[updates[0]]);
    assert_eq!(first.updates[0].item.equipped, 0);
    assert_eq!(first.updates[0].change, InventoryUpdate::MODIFIED);
}

#[test]
fn destroy_refusals() {
    let (mut world, log) = world_with(player(vec![inv(100, 22), inv(101, 2)]));
    let destroy = |world: &mut World, object_id: i32, count: i64| {
        input(world, ClientInput::DestroyItem { object_id, count });
    };
    destroy(&mut world, 100, 1);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::ThisItemCannotBeDiscarded
    );
    destroy(&mut world, 999, 1);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::ThisItemCannotBeDiscarded
    );
    destroy(&mut world, 101, 0);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::YouCannotDestroyItBecauseTheNumberIsIncorrect
    );
    destroy(&mut world, 101, -5);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::YouCannotDestroyItBecauseTheNumberIsIncorrect
    );
    // A non-stackable item with count 2 is an exploit attempt: logged, nothing sent.
    destroy(&mut world, 101, 2);
    assert!(log.take().is_empty());
    assert_eq!(world.player(ME).unwrap().data.items.len(), 2);
}

#[test]
fn destroying_an_item_removes_its_shortcut() {
    let mut p = player(vec![inv(100, 2), inv(101, 3)]);
    p.shortcuts = vec![
        ShortcutData {
            slot: 0,
            page: 0,
            kind: 1,
            shortcut_id: 100,
            level: 0,
        },
        ShortcutData {
            slot: 1,
            page: 0,
            kind: 1,
            shortcut_id: 101,
            level: 0,
        },
    ];
    let (mut world, log) = world_with(p);
    input(
        &mut world,
        ClientInput::DestroyItem {
            object_id: 100,
            count: 1,
        },
    );
    let packets = log.packets();
    let init: ShortcutInit = decode(find(&packets, "SHORT_CUT_INIT")[0]);
    assert_eq!(init.shortcuts.len(), 1);
    assert_eq!(world.player(ME).unwrap().data.shortcuts.len(), 1);
}

// ---- dropping and the item list ---------------------------------------------------------------------

#[test]
fn dropping_refuses_what_java_refuses_and_puts_the_rest_on_the_ground() {
    let (mut world, log) = world_with(player(vec![worn(100, 2, paperdoll::RHAND), inv(101, 3)]));
    let drop = |world: &mut World, object_id: i32, count: i64, x: i32, z: i32| {
        input(
            world,
            ClientInput::DropItem {
                object_id,
                count,
                x,
                y: 1000,
                z,
            },
        );
    };
    // Every refusal of Java's checks first.
    drop(&mut world, 101, 0, 1000, 0);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::ThisItemCannotBeDiscarded
    );
    drop(&mut world, 101, 5, 1000, 0);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::ThisItemCannotBeDiscarded
    );
    // Too far away: 150 and 50 are the limits.
    drop(&mut world, 101, 1, 1150, 0);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::YouCannotDiscardSomethingThatFarAwayFromYou
    );
    drop(&mut world, 101, 1, 1000, 51);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::YouCannotDiscardSomethingThatFarAwayFromYou
    );
    // Valid requests drop the item (M4-DROP-01): `Player.dropItem` takes it out of the inventory and
    // `dropMe` puts it on the ground.
    drop(&mut world, 101, 1, 1149, 50);
    drop(&mut world, 100, 1, 1000, 0);
    let out = log.take();
    // The only message: the grade penalty of the dropped sword ends ("The effect of $s1 has
    // been removed.", `removeSkill(.., true)` of the passive penalty skill).
    let messages = system_messages_of(&out);
    assert_eq!(
        messages.iter().map(|m| m.id).collect::<Vec<_>>(),
        [SystemMessageId::TheEffectOfS1HasBeenRemoved]
    );
    // The rows are deleted at once (`Item.updateDatabase()` of an item that has no owner); the
    // sword is written as taken off first.
    assert_eq!(
        item_changes(&out),
        [
            (vec![], vec![101]),
            (vec![(100, "INVENTORY".to_string(), 0)], vec![]),
            (vec![], vec![100]),
        ]
    );
    assert!(worn_slots(&world).is_empty());
    assert!(world.player(ME).unwrap().data.items.is_empty());
    assert_eq!(world.ground_item_ids(), [ObjectId(100), ObjectId(101)]);
    // `DropItem` for the dropper's neighbours is `broadcast`; the dropper hears about the loss of
    // the item through the delayed `InventoryUpdate`.
    advance(&mut world, 200);
    let packets = names(&log.packets());
    assert!(packets.contains(&"INVENTORY_UPDATE".to_string()), "{packets:?}");
}

fn system_messages_of(out: &[Out]) -> Vec<SystemMessage> {
    let packets: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(_, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    system_messages(&packets)
}

#[test]
fn undroppable_and_quest_items_are_refused_with_their_messages() {
    let (mut world, log) = world_with(player(vec![inv(100, 22), item(101, 21, 3, "INVENTORY", 0)]));
    let drop = |world: &mut World, object_id: i32| {
        input(
            world,
            ClientInput::DropItem {
                object_id,
                count: 1,
                x: 1000,
                y: 1000,
                z: 0,
            },
        );
    };
    drop(&mut world, 100);
    assert_eq!(
        system_messages(&log.packets())[0].id,
        SystemMessageId::ThisItemCannotBeDiscarded
    );
    // A quest item: no message (`if (item.isQuestItem() && ...) return;`).
    drop(&mut world, 101);
    assert!(log.take().is_empty());
}

#[test]
fn the_inventory_half_of_a_drop_splits_a_stack_and_writes_it() {
    let (mut world, log) = world_with(player(vec![item(100, 23, 10, "INVENTORY", 0)]));
    let dropped = world
        .drop_from_inventory(ME, ObjectId(100), 3, ObjectId(0x6000_0001))
        .unwrap();
    assert_eq!(
        (dropped.object_id.0, dropped.count, dropped.loc.as_str()),
        (0x6000_0001, 3, "VOID")
    );
    assert_eq!(world.player(ME).unwrap().data.items[0].count, 7);
    let out = log.take();
    // The smaller stack is written at once (`dropItem` -> `item.updateDatabase()`).
    assert_eq!(
        item_changes(&out),
        [(vec![(100, "INVENTORY".to_string(), 0)], vec![])]
    );
    advance(&mut world, 100);
    let packets = log.packets();
    assert_eq!(names(&packets), ["INVENTORY_UPDATE"]);

    // The whole stack: the row is deleted.
    let dropped = world
        .drop_from_inventory(ME, ObjectId(100), 7, ObjectId(0x6000_0002))
        .unwrap();
    assert_eq!((dropped.object_id.0, dropped.count), (100, 7));
    assert!(world.player(ME).unwrap().data.items.is_empty());
    assert_eq!(item_changes(&log.take()), [(vec![], vec![100])]);
}

#[test]
fn the_item_list_comes_250_ms_after_the_request() {
    let (mut world, log) = world_with(player(vec![inv(100, 2), item(101, 21, 3, "INVENTORY", 0)]));
    input(&mut world, ClientInput::ItemList);
    let early = log.take();
    assert!(early.is_empty(), "{early:?}");
    advance(&mut world, 200);
    let early = log.take();
    assert!(
        early.is_empty(),
        "{:?}",
        early
            .iter()
            .map(|o| match o {
                Out::Packet(_, b) => name_of(b),
                _ => "persist".into(),
            })
            .collect::<Vec<_>>()
    );
    advance(&mut world, 100);
    let packets = log.packets();
    // ItemList.runImpl: the quest items follow.
    assert_eq!(names(&packets), ["ITEM_LIST", "EX_QUEST_ITEM_LIST"]);
    let list: ItemList = decode(&packets[0]);
    assert!(list.show_window);
    assert_eq!(list.items.len(), 1);
    let quest: ExQuestItemList = decode(&packets[1]);
    assert_eq!(quest.items.len(), 1);
    // Two requests in a row make one list (the second replaces the first).
    input(&mut world, ClientInput::ItemList);
    input(&mut world, ClientInput::ItemList);
    advance(&mut world, 400);
    assert_eq!(find(&log.packets(), "ITEM_LIST").len(), 1);
}

#[test]
fn the_delayed_inventory_update_lists_items_in_java_hash_map_order() {
    // Object ids 16, 17, 18 land in buckets 0, 1, 2 of a 16-bucket HashMap, whatever order they
    // were put in.
    let mut p = player(vec![inv(18, 6), inv(16, 6), inv(17, 6)]);
    p.skills = vec![];
    let (mut world, log) = world_with(p);
    for object_id in [18, 16, 17] {
        use_item(&mut world, object_id);
    }
    log.take();
    advance(&mut world, 200);
    let packets = log.packets();
    let iu: InventoryUpdate = decode(find(&packets, "INVENTORY_UPDATE")[0]);
    let ids: Vec<i32> = iu.updates.iter().map(|u| u.item.object_id).collect();
    assert_eq!(ids, [16, 17, 18]);
}

#[test]
fn equipping_does_not_touch_other_players_state() {
    // A second player in sight gets the CharInfo of the first 100 ms after the change.
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
    world.set_static_data(Box::new(data()));
    let mut me = player(vec![inv(100, 2)]);
    me.name = "Me".into();
    let mut other = player(vec![]);
    other.object_id = ObjectId(2);
    other.name = "Other".into();
    other.x = 1010;
    for (session, p) in [(1u64, me), (2, other)] {
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(session),
            player: Box::new(p),
            sink: Box::new(Sink(i32::try_from(session).unwrap(), log.clone())),
            ip: None,
        });
    }
    log.take();
    use_item(&mut world, 100);
    let out = log.take();
    // The other player already got the attack speed StatusUpdate (broadcastPacket).
    let to_other: Vec<Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(2, b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names(&to_other), ["STATUS_UPDATE"]);
    advance(&mut world, 100);
    let out = log.take();
    let to_other: Vec<String> = out
        .iter()
        .filter_map(|o| match o {
            Out::Packet(2, b) => Some(name_of(b)),
            _ => None,
        })
        .collect();
    assert!(to_other.contains(&"CHAR_INFO".to_string()), "{to_other:?}");
}

// ---- limits ---------------------------------------------------------------------------------------

#[test]
fn slot_and_weight_limits() {
    // 80 slots for a non-dwarf: 79 items leave room for one.
    let items: Vec<ItemData> = (0..79).map(|i| inv(1000 + i, 23)).collect();
    let (world, _log) = world_with(player(items));
    assert!(world.validate_capacity(ME, 1, false));
    assert!(!world.validate_capacity(ME, 2, false));
    // A quest item is counted against the quest limit (100), not the 80.
    assert!(world.validate_capacity(ME, 2, true));
    // NOTE(java-quirk): zero slots are checked against the quest limit unless AutoLootSlotLimit.
    assert!(world.validate_capacity(ME, 0, false));
    // A stackable item the player has needs no new slot.
    assert!(world.validate_capacity_by_item_id(ME, 23));
    assert!(world.validate_capacity_by_item_id(ME, 2));

    let max_load = i64::from(world.stats(ME).unwrap().max_load());
    let load = i64::from(world.stats(ME).unwrap().current_load());
    assert_eq!(load, 79 * 2);
    assert!(world.validate_weight(ME, max_load - load));
    assert!(!world.validate_weight(ME, max_load - load + 1));
    // 1000 per anvil.
    assert!(world.validate_weight_by_item_id(ME, 20, (max_load - load) / 1000));
    assert!(!world.validate_weight_by_item_id(ME, 20, (max_load - load) / 1000 + 1));
    // An item the datapack does not know always passes.
    assert!(world.validate_weight_by_item_id(ME, 999_999, 1_000_000_000));
}
