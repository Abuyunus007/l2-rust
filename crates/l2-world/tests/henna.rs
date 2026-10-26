#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Hennas and the symbol maker (L-01): drawing and removing a dye with Java's fees and messages,
//! the stat change, the `character_hennas` writes, the lists and previews, and the `SymbolMaker`
//! pages. Expected values come from `RequestHennaEquip.java`, `RequestHennaRemove.java`,
//! `Player.addHenna/removeHenna/getHennaEmptySlots`, `HennaEquipList.java`, `HennaRemoveList.java`,
//! `HennaItemDrawInfo.java`, `HennaItemRemoveInfo.java` and `SymbolMaker.java`.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, MapRegions, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillLoadOptions, SkillTemplates, SpawnData, SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::henna::{HennaEquipList, HennaItemInfo, HennaItemRemoveInfo, HennaRemoveList};
use l2_protocol_game::server::{HennaInfo, NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- harness -----------------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Out {
    Packet(Vec<u8>),
    Persist(PersistCommand),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.0.0.lock().unwrap().push(Out::Packet(bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct Recorder(Log);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

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
            shared_reuse_group: -1,
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

fn packets<'a>(out: &'a [Out], name: &str) -> Vec<&'a [u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(b) if name_of(b) == name => Some(b.as_slice()),
            _ => None,
        })
        .collect()
}

fn decoded<P: Decode>(out: &[Out], name: &str) -> Vec<P> {
    packets(out, name)
        .into_iter()
        .map(|raw| {
            let skip = if raw[0] == 0xFE { 3 } else { 1 };
            P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
        })
        .collect()
}

fn message_ids(out: &[Out]) -> Vec<SystemMessageId> {
    decoded::<SystemMessage>(out, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|m| m.id)
        .collect()
}

fn names(out: &[Out]) -> Vec<String> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(b) => Some(name_of(b)),
            Out::Persist(_) => None,
        })
        .collect()
}

fn henna_writes(out: &[Out]) -> Vec<HennaWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Henna(w)) => Some(*w),
            _ => None,
        })
        .collect()
}

// ---- data --------------------------------------------------------------------------------------

/// Class 2 (Gladiator) is level 2: three slots. Class 1 (Warrior) is level 1: two slots. Class 0
/// (Human Fighter) is level 0: none.
fn template(class_id: i32) -> String {
    format!(
        r#"<list><classId>{class_id}</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#
    )
}

const ADENA: i32 = 57;
const DYE_A: i32 = 4445;
const DYE_B: i32 = 4446;
const DYE_C: i32 = 4447;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="4445" type="EtcItem" name="Dye A"><set name="is_stackable" val="true" /></item>
<item id="4446" type="EtcItem" name="Dye B"><set name="is_stackable" val="true" /></item>
<item id="4447" type="EtcItem" name="Dye C"><set name="is_stackable" val="true" /></item>
</list>"#;

/// Dye 1: STR +2 CON -1, 10 dyes and 37000 adena to draw, 5 dyes and 7400 adena to remove.
/// Dye 2: STR +4 (the cap of 5 per stat applies with dye 1), cheap. Dye 3: only for class 0.
const HENNAS: &str = r#"<list>
<henna dyeId="1" dyeName="a" dyeItemId="4445" dyeLevel="1"><stats str="2" con="-1" dex="0" int="0" men="0" wit="0" />
<wear count="10" fee="37000" /><cancel count="5" fee="7400" /><classId>1</classId><classId>2</classId></henna>
<henna dyeId="2" dyeName="b" dyeItemId="4446" dyeLevel="1"><stats str="4" con="0" dex="0" int="0" men="0" wit="0" />
<wear count="1" fee="100" /><cancel count="1" fee="50" /><classId>1</classId><classId>2</classId></henna>
<henna dyeId="3" dyeName="c" dyeItemId="4447" dyeLevel="1"><stats str="0" con="0" dex="1" int="0" men="0" wit="0" />
<wear count="1" fee="100" /><cancel count="1" fee="50" /><classId>0</classId></henna>
</list>"#;

fn stat_data(class_id: i32) -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&template(class_id), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::from_xml(HENNAS).unwrap()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            "<list></list>",
            &l2_data::skill::EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

const NPCS: &str = r#"<list>
  <npc id="31046" type="Folk" level="10" name="Marsden"/>
</list>"#;

const PAGE: &str = "<html><body>Symbols %objectId% %playername% \
<a action=\"bypass -h Script SymbolMaker Draw\">Draw</a>\
<a action=\"bypass -h Script SymbolMaker Remove\">Remove</a>\
<a action=\"bypass Script SymbolMaker symbol_maker-1.htm\">More</a></body></html>";
const PAGE_1: &str = "<html><body>More about symbols %objectId%</body></html>";

const ME: ObjectId = ObjectId(0x2000_0001);
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

fn item(object_id: i32, item_id: i32, slot: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".into(),
        loc_data: slot,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    /// A character of `class_id` with 100000 adena and 25 dyes A, 3 dyes B, 2 dyes C.
    fn new(class_id: i32, hennas: [i32; 3]) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        let script = "data/scripts/ai/others/SymbolMaker";
        write(&format!("{script}/symbol_maker.htm"), PAGE);
        write(&format!("{script}/symbol_maker-1.htm"), PAGE_1);

        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(20_261_027), Box::new(Recorder(log.clone())));
        world.set_static_data(Box::new(Data(stat_data(class_id))));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        let profile = PlayerProfile {
            hennas,
            ..PlayerProfile::default()
        };
        let data = PlayerData {
            object_id: ME,
            account: "acc".into(),
            name: "Hero".into(),
            title: String::new(),
            class_id,
            base_class: class_id,
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
            access_level: 0,
            x: X,
            y: Y,
            z: Z,
            heading: 0,
            cur_hp: 100.0,
            max_hp: 100.0,
            cur_mp: 50.0,
            max_mp: 50.0,
            cur_cp: 20.0,
            max_cp: 20.0,
            online_time: 0,
            items: vec![
                item(0x3000_0001, ADENA, 0, 100_000),
                item(0x3000_0002, DYE_A, 1, 25),
                item(0x3000_0003, DYE_B, 2, 3),
                item(0x3000_0004, DYE_C, 3, 2),
            ],
            skills: vec![],
            shortcuts: vec![],
            profile,
        };
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(data),
            sink: Box::new(Sink(log.clone())),
            ip: None,
        });
        for _ in 0..20 {
            world.tick();
        }
        log.take();
        Self {
            world,
            log,
            _root: root,
        }
    }

    fn input(&mut self, input: ClientInput) -> Vec<Out> {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input,
        });
        // The inventory update goes out 100 ms after the change.
        for _ in 0..5 {
            self.world.tick();
        }
        self.log.take()
    }

    fn henna(&mut self, input: HennaInput) -> Vec<Out> {
        self.input(ClientInput::Henna(input))
    }

    /// The transaction flood protector allows one request per tick window; let it pass.
    fn wait(&mut self) {
        for _ in 0..40 {
            self.world.tick();
        }
        self.log.take();
    }

    fn count(&self, item_id: i32) -> i64 {
        self.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id && i.loc == "INVENTORY")
            .map(|i| i.count)
            .sum()
    }

    fn slots(&self) -> [i32; 3] {
        self.world.player(ME).unwrap().data.profile.hennas
    }

    fn str(&self) -> i32 {
        self.world.stats(ME).unwrap().str()
    }
}

// ---- drawing -----------------------------------------------------------------------------------

/// `RequestHennaEquip`: the dye takes the first empty slot, the dyes and the adena are paid, the
/// client gets `HennaInfo`, `UserInfo` and the messages; the row is written with slot 1.
#[test]
fn equip_pays_the_fee_fills_the_slot_and_writes_the_row() {
    let mut s = Scene::new(2, [0; 3]);
    let before = s.str();
    let out = s.henna(HennaInput::Equip { symbol_id: 1 });
    assert_eq!(s.slots(), [1, 0, 0]);
    assert_eq!(s.count(DYE_A), 15);
    assert_eq!(s.count(ADENA), 63_000);
    assert_eq!(s.str(), before + 2);
    assert_eq!(
        henna_writes(&out),
        [HennaWrite::Add {
            char_id: ME.0,
            symbol_id: 1,
            slot: 1,
            class_index: 0
        }]
    );
    // addHenna sends HennaInfo then UserInfo before the dyes are taken.
    let order = names(&out);
    let at = |name: &str| order.iter().position(|n| n == name).unwrap();
    assert!(at("HENNA_INFO") < at("USER_INFO"));
    assert!(at("USER_INFO") < at("SYSTEM_MESSAGE"));
    let info = &decoded::<HennaInfo>(&out, "HENNA_INFO")[0];
    assert_eq!((info.str, info.con, info.dye_ids.clone()), (2, -1, vec![1]));
    assert_eq!(
        message_ids(&out),
        [
            SystemMessageId::S2S1HasDisappeared,
            SystemMessageId::TheSymbolHasBeenAdded
        ]
    );
    let gone = &decoded::<SystemMessage>(&out, "SYSTEM_MESSAGE")[0];
    assert_eq!(
        gone.params,
        [
            SystemMessageParam::ItemName(DYE_A),
            SystemMessageParam::LongNumber(10)
        ]
    );
    assert!(!names(&out).contains(&"ACTION_FAIL".to_owned()));
}

/// The three slots fill in order, a character of a level 1 class has two, the cap of 5 per stat
/// applies to the sum (`recalcHennaStats`).
#[test]
fn slots_fill_in_order_and_a_full_character_is_refused() {
    let mut s = Scene::new(1, [0; 3]);
    s.henna(HennaInput::Equip { symbol_id: 1 });
    s.wait();
    let base = s.str() - 2;
    let out = s.henna(HennaInput::Equip { symbol_id: 2 });
    assert_eq!(s.slots(), [1, 2, 0]);
    // 2 + 4 = 6 is above 5, so the second dye only adds 3.
    assert_eq!(s.str(), base + 5);
    assert_eq!(
        henna_writes(&out),
        [HennaWrite::Add {
            char_id: ME.0,
            symbol_id: 2,
            slot: 2,
            class_index: 0
        }]
    );
    // Warrior has two slots: the third dye is refused with the slot message and ActionFailed.
    s.wait();
    let out = s.henna(HennaInput::Equip { symbol_id: 2 });
    assert_eq!(message_ids(&out), [SystemMessageId::NoSlotExistsToDrawTheSymbol]);
    assert!(names(&out).contains(&"ACTION_FAIL".to_owned()));
    assert!(henna_writes(&out).is_empty());
    assert_eq!(s.slots(), [1, 2, 0]);
}

/// A character of a class of level 0 has no slot at all.
#[test]
fn a_level_zero_class_has_no_slot() {
    let mut s = Scene::new(0, [0; 3]);
    let out = s.henna(HennaInput::Equip { symbol_id: 3 });
    assert_eq!(message_ids(&out), [SystemMessageId::NoSlotExistsToDrawTheSymbol]);
    assert_eq!(s.slots(), [0; 3]);
}

/// Not enough dyes, not enough adena, an unknown dye, a dye of another class: nothing is paid and
/// no slot changes. The forbidden dye also counts as an illegal action (a non-GM).
#[test]
fn equip_refusals_change_nothing() {
    let mut s = Scene::new(2, [0; 3]);
    // Dye B is plentiful, dye C only for class 0.
    let out = s.henna(HennaInput::Equip { symbol_id: 99 });
    assert!(
        message_ids(&out).is_empty(),
        "an unknown dye id only fails the action"
    );
    assert_eq!(names(&out), ["ACTION_FAIL"]);

    s.wait();
    let out = s.henna(HennaInput::Equip { symbol_id: 3 });
    assert_eq!(message_ids(&out)[0], SystemMessageId::TheSymbolCannotBeDrawn);
    assert!(names(&out).contains(&"ACTION_FAIL".to_owned()));
    assert_eq!(s.slots(), [0; 3]);
    assert_eq!((s.count(DYE_C), s.count(ADENA)), (2, 100_000));
    assert!(henna_writes(&out).is_empty());
}

#[test]
fn equip_without_the_dyes_or_the_adena_is_refused() {
    let mut s = Scene::new(2, [0; 3]);
    // Dye A needs 10: take 20 away, 5 are left.
    s.world.destroy_item_by_item_id(ME, DYE_A, 20, false);
    s.log.take();
    let out = s.henna(HennaInput::Equip { symbol_id: 1 });
    assert_eq!(message_ids(&out), [SystemMessageId::TheSymbolCannotBeDrawn]);
    assert_eq!((s.slots(), s.count(DYE_A)), ([0; 3], 5));

    s.wait();
    let mut s = Scene::new(2, [0; 3]);
    s.world.reduce_adena(ME, 70_000, false);
    s.log.take();
    let out = s.henna(HennaInput::Equip { symbol_id: 1 });
    assert_eq!(message_ids(&out), [SystemMessageId::TheSymbolCannotBeDrawn]);
    assert_eq!((s.slots(), s.count(ADENA)), ([0; 3], 30_000));
}

// ---- removing ----------------------------------------------------------------------------------

/// `RequestHennaRemove`: the slot is emptied and its row deleted, the dyes come back, the fee goes,
/// and the client gets HennaInfo, UserInfo, "You have earned" and "The symbol has been deleted".
#[test]
fn remove_refunds_the_dyes_takes_the_fee_and_deletes_the_row() {
    let mut s = Scene::new(2, [0, 1, 0]);
    let with_dye = s.str();
    let out = s.henna(HennaInput::Remove { symbol_id: 1 });
    assert_eq!(s.slots(), [0; 3]);
    assert_eq!(s.count(DYE_A), 30);
    assert_eq!(s.count(ADENA), 100_000 - 7_400);
    assert_eq!(s.str(), with_dye - 2);
    assert_eq!(
        henna_writes(&out),
        [HennaWrite::Delete {
            char_id: ME.0,
            slot: 2,
            class_index: 0
        }]
    );
    assert_eq!(
        message_ids(&out),
        [
            SystemMessageId::YouHaveEarnedS2S1S,
            SystemMessageId::TheSymbolHasBeenDeleted
        ]
    );
    let earned = &decoded::<SystemMessage>(&out, "SYSTEM_MESSAGE")[0];
    assert_eq!(
        earned.params,
        [
            SystemMessageParam::ItemName(DYE_A),
            SystemMessageParam::LongNumber(5)
        ]
    );
    let info = &decoded::<HennaInfo>(&out, "HENNA_INFO")[0];
    assert!(info.dye_ids.is_empty());
}

#[test]
fn remove_without_the_adena_or_the_dye_is_refused() {
    let mut s = Scene::new(2, [1, 0, 0]);
    s.world.reduce_adena(ME, 99_000, false);
    s.log.take();
    let out = s.henna(HennaInput::Remove { symbol_id: 1 });
    assert_eq!(message_ids(&out), [SystemMessageId::YouDoNotHaveEnoughAdena]);
    assert!(names(&out).contains(&"ACTION_FAIL".to_owned()));
    assert_eq!(s.slots(), [1, 0, 0]);
    assert!(henna_writes(&out).is_empty());

    // A dye the character does not wear: only ActionFailed.
    s.wait();
    let out = s.henna(HennaInput::Remove { symbol_id: 2 });
    assert_eq!(names(&out), ["ACTION_FAIL"]);
    assert_eq!(s.slots(), [1, 0, 0]);
}

// ---- lists and previews ------------------------------------------------------------------------

/// `HennaEquipList`: Java writes the size of the class's whole list but only the dyes the character
/// has an item of.
#[test]
fn the_draw_list_shows_the_dyes_the_character_carries() {
    let mut s = Scene::new(2, [0; 3]);
    let out = s.henna(HennaInput::ItemList);
    let list = &decoded::<HennaEquipList>(&out, "HENNA_EQUIP_LIST")[0];
    assert_eq!(list.adena, 100_000);
    // Dye 3 is not for class 2: the class list has two dyes.
    assert_eq!(list.total, 2);
    let rows: Vec<(i32, i32, i64, i64, bool)> = list
        .entries
        .iter()
        .map(|e| (e.dye_id, e.dye_item_id, e.wear_count, e.wear_fee, e.allowed))
        .collect();
    assert_eq!(rows, [(1, DYE_A, 10, 37_000, true), (2, DYE_B, 1, 100, true)]);

    // Without dye B in the inventory its row is left out, the count stays 2.
    s.world.destroy_item_by_item_id(ME, DYE_B, -1, false);
    s.log.take();
    let out = s.henna(HennaInput::ItemList);
    let list = &decoded::<HennaEquipList>(&out, "HENNA_EQUIP_LIST")[0];
    assert_eq!((list.total, list.entries.len()), (2, 1));
}

/// `HennaRemoveList`: the count is `3 - getHennaEmptySlots()`, which is not the number of rows for a
/// class with fewer than three slots.
#[test]
fn the_remove_list_count_is_three_minus_the_empty_slots() {
    let mut s = Scene::new(2, [1, 0, 2]);
    let out = s.henna(HennaInput::RemoveList);
    let list = &decoded::<HennaRemoveList>(&out, "HENNA_UNEQUIP_LIST")[0];
    assert_eq!((list.adena, list.count), (100_000, 2));
    let rows: Vec<(i32, i32, i32, i32)> = list
        .entries
        .iter()
        .map(|e| (e.dye_id, e.dye_item_id, e.cancel_count, e.cancel_fee))
        .collect();
    assert_eq!(rows, [(1, DYE_A, 5, 7_400), (2, DYE_B, 1, 50)]);

    // A Warrior (two slots) wearing one dye: one empty slot, so the count says 2 for 1 row.
    let mut s = Scene::new(1, [1, 0, 0]);
    let out = s.henna(HennaInput::RemoveList);
    let list = &decoded::<HennaRemoveList>(&out, "HENNA_UNEQUIP_LIST")[0];
    assert_eq!((list.count, list.entries.len()), (2, 1));
}

/// `HennaItemDrawInfo` adds the dye's bonus to the current stat, `HennaItemRemoveInfo` subtracts it.
#[test]
fn the_previews_show_the_stats_with_the_dye_added_or_taken_off() {
    let mut s = Scene::new(2, [1, 0, 0]);
    let now = s.str();
    let out = s.henna(HennaInput::ItemInfo { symbol_id: 2 });
    let HennaItemInfo(preview) = &decoded::<HennaItemInfo>(&out, "HENNA_ITEM_INFO")[0];
    assert_eq!((preview.dye_id, preview.dye_item_id), (2, DYE_B));
    assert_eq!((preview.count, preview.fee, preview.allowed), (1, 100, true));
    assert_eq!(preview.adena, 100_000);
    // The preview is not capped: the current STR plus the whole bonus.
    assert_eq!((preview.str.now, i32::from(preview.str.after)), (now, now + 4));
    assert_eq!(preview.con.now, 43 - 1);

    let out = s.henna(HennaInput::ItemRemoveInfo { symbol_id: 1 });
    let HennaItemRemoveInfo(preview) = &decoded::<HennaItemRemoveInfo>(&out, "HENNA_UNEQUIP_INFO")[0];
    assert_eq!((preview.count, preview.fee), (5, 7_400));
    assert_eq!((preview.str.now, i32::from(preview.str.after)), (now, now - 2));
    assert_eq!(i32::from(preview.con.after), preview.con.now + 1);

    // An unknown dye: ActionFailed (a zero id is not even logged); remove info of 0 is ignored.
    let out = s.henna(HennaInput::ItemInfo { symbol_id: 0 });
    assert_eq!(names(&out), ["ACTION_FAIL"]);
    let out = s.henna(HennaInput::ItemRemoveInfo { symbol_id: 0 });
    assert!(out.is_empty());
    let out = s.henna(HennaInput::ItemRemoveInfo { symbol_id: 99 });
    assert_eq!(names(&out), ["ACTION_FAIL"]);
}

// ---- the symbol maker ---------------------------------------------------------------------------

fn html_of(out: &[Out]) -> Vec<NpcHtmlMessage> {
    decoded::<NpcHtmlMessage>(out, "NPC_HTML_MESSAGE")
}

/// `SymbolMaker.onFirstTalk` shows `symbol_maker.htm`; the `Script SymbolMaker` links open a page
/// or the draw and remove lists.
#[test]
fn the_symbol_maker_pages_and_lists() {
    let mut s = Scene::new(2, [2, 0, 0]);
    let npc = s.world.spawn_npc_at(31046, X + 20, Y, Z, 0).unwrap();
    let talk = |s: &mut Scene| {
        let mut all = Vec::new();
        for _ in 0..2 {
            all.extend(s.input(ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            }));
        }
        all
    };
    let out = talk(&mut s);
    let pages = html_of(&out);
    assert_eq!(pages.len(), 1, "{:?}", names(&out));
    assert!(
        pages[0]
            .html
            .starts_with(&format!("<html><body>Symbols {} Hero ", npc.0))
    );

    s.wait();
    let out = s.input(ClientInput::Bypass {
        command: "Script SymbolMaker Draw".to_owned(),
    });
    assert_eq!(names(&out), ["HENNA_EQUIP_LIST"]);

    s.wait();
    let out = s.input(ClientInput::Bypass {
        command: "Script SymbolMaker Remove".to_owned(),
    });
    let list = &decoded::<HennaRemoveList>(&out, "HENNA_UNEQUIP_LIST")[0];
    assert_eq!(list.entries.len(), 1);

    // The page replaces the links of the first one, so it comes last.
    s.wait();
    let out = s.input(ClientInput::Bypass {
        command: "Script SymbolMaker symbol_maker-1.htm".to_owned(),
    });
    assert_eq!(
        html_of(&out)[0].html,
        format!("<html><body>More about symbols {}</body></html>", npc.0)
    );
}
