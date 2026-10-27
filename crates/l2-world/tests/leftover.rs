#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The client leftovers of L-02: macros, the crystallize, the wear shop, item links in the chat,
//! the key mapping, the inventory order, the mini map, "record info", rotating, the skill list,
//! the dispel of a buff and a skill on the ground. The expected values come from the Java code
//! named in each test (`RequestMakeMacro`, `MacroList`, `RequestCrystallizeItem`,
//! `RequestPreviewItem`, `RequestExRqItemLink`, `RequestSaveKeyMapping`,
//! `RequestSaveInventoryOrder`, `RequestShowMiniMap`, `RequestRecordInfo`, `StartRotating`,
//! `FinishRotating`, `RequestSkillList`, `RequestDispel`, `RequestExMagicSkillUseGround`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, BuyListOptions, BuyLists, EnchantHpBonus, Hennas, HtmCache, ItemTemplates,
    MerchantPriceConfigs, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates,
    SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::client::{
    MacroCommand, RequestCrystallizeItem, RequestDispel, RequestExMagicSkillUseGround, RequestMakeMacro,
    RequestPreviewItem,
};
use l2_protocol_game::server::{
    ExRpItemLink, MacroList, ShopPreviewInfo, ShowMiniMap, StartRotation, StopRotation, SystemMessage,
    SystemMessageParam, ValidateLocation,
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

const ADENA: i32 = 57;
const SWORD: i32 = 2;
const SWORD_2: i32 = 6;
const CUIRASS: i32 = 4;
const HAT: i32 = 5;
const B_SWORD: i32 = 10;
const A_SWORD: i32 = 11;
const HERO_SWORD: i32 = 6611;
const SHADOW_SWORD: i32 = 12;
const PLAIN_SWORD: i32 = 13;
const CRYSTAL_B: i32 = 1460;
const CRYSTAL_A: i32 = 1461;

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="C Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /><set name="price" val="1000" /></item>
<item id="6" type="Weapon" name="C Sword 2"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /><set name="price" val="1000" /></item>
<item id="4" type="Armor" name="Cuirass"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="weight" val="10" /><set name="price" val="2000" /></item>
<item id="5" type="Armor" name="Hat"><set name="armor_type" val="LIGHT" /><set name="bodypart" val="head" /><set name="weight" val="2" /><set name="price" val="400" /></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /><set name="price" val="1" /></item>
<item id="10" type="Weapon" name="B Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /><set name="crystal_type" val="B" /><set name="crystal_count" val="100" /></item>
<item id="11" type="Weapon" name="A Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /><set name="crystal_type" val="A" /><set name="crystal_count" val="300" /></item>
<item id="6611" type="Weapon" name="Hero Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /><set name="crystal_type" val="B" /><set name="crystal_count" val="100" /></item>
<item id="12" type="Weapon" name="Shadow Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /><set name="crystal_type" val="B" /><set name="crystal_count" val="100" /></item>
<item id="13" type="Weapon" name="Plain Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /></item>
<item id="1460" type="EtcItem" name="Crystal: B-Grade"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="1461" type="EtcItem" name="Crystal: A-Grade"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
</list>"#;

const SKILLS: &str = r##"<list>
<skill id="248" levels="5" name="Crystallize"><operateType>P</operateType><targetType>SELF</targetType></skill>
<skill id="9001" levels="1" name="Might"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9020" levels="1" name="Dance A"><operateType>A2</operateType><targetType>SELF</targetType><isMagic>3</isMagic>
<abnormalTime>60</abnormalTime><abnormalType>CRITICAL_DMG_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="critDmg" val="1.1"/></effect></effects></skill>
<skill id="9040" levels="1" name="Weakness"><operateType>A2</operateType><targetType>SELF</targetType><isDebuff>true</isDebuff>
<abnormalTime>30</abnormalTime><abnormalType>POISON</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="0.5"/></effect></effects></skill>
<skill id="9060" levels="1" name="Lasting"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>MD_UP</abnormalType><abnormalLevel>1</abnormalLevel><stayAfterDeath>true</stayAfterDeath>
<effects><effect name="Buff"><mul stat="mDef" val="1.1"/></effect></effects></skill>
<skill id="9070" levels="1" name="Fixed"><operateType>A2</operateType><targetType>SELF</targetType><canBeDispeled>false</canBeDispeled>
<abnormalTime>60</abnormalTime><abnormalType>SPEED_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="runSpd" val="1.1"/></effect></effects></skill>
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
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
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
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="30001" type="Merchant" level="10" name="Trader" usingServerSideName="true"/>
  <npc id="30003" type="Folk" level="10" name="Villager" usingServerSideName="true"/>
</list>"#;

/// List 100 is Trader's.
const BUY_LISTS: [(&str, &str); 1] = [(
    "0000100.xml",
    r#"<list><npcs><npc>30001</npc></npcs>
        <item id="2" price="1500" />
        <item id="6" price="1500" />
        <item id="4" price="2000" />
        <item id="5" price="400" />
    </list>"#,
)];

const PRICE_CONFIGS: &str = r#"<merchantPriceConfig defaultPriceConfig="18">
    <priceConfig id="18" name="Neutral" baseTax="50" />
</merchantPriceConfig>"#;

// ---- helpers ------------------------------------------------------------------------------------

const A: ObjectId = ObjectId(1);
const B: ObjectId = ObjectId(2);
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

/// What `handleIllegalPlayerAction` tells the player with the default `Kick` punishment.
const KICK: &str = "You will be kicked for illegal action, GM informed.";

fn item(object_id: i32, item_id: i32, count: i64, loc: &str, loc_data: i32) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: loc.into(),
        loc_data,
        ..Default::default()
    }
}

fn inv(object_id: i32, item_id: i32, count: i64) -> ItemData {
    item(object_id, item_id, count, "INVENTORY", 0)
}

fn player(id: ObjectId, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: format!("P{}", id.0),
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
        x: X,
        y: Y,
        z: Z,
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

struct Root(PathBuf);

impl Root {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-leftover-{tag}-{}", std::process::id()));
        let write = |path: &str, text: &str| {
            let file = dir.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        write(
            "data/html/npcdefault.htm",
            "<html><body>I have nothing to say to you.</body></html>",
        );
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Scene {
    world: World,
    log: Log,
    _root: Root,
}

impl Scene {
    fn new(tag: &str) -> Self {
        Self::with(tag, WorldSettings::default())
    }

    fn with(tag: &str, settings: WorldSettings) -> Self {
        let root = Root::new(tag);
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
        let data = data();
        let items = Arc::clone(&data.items);
        world.set_static_data(Box::new(data));
        world.set_settings(settings);
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        let lists = BuyLists::from_xml(&BUY_LISTS, &items, &BuyListOptions::default());
        world.set_buy_lists(
            Arc::new(lists),
            Arc::new(MerchantPriceConfigs::from_xml(PRICE_CONFIGS).unwrap()),
        );
        Self {
            world,
            log,
            _root: root,
        }
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.advance(20);
        self.log.take();
    }

    fn input(&mut self, who: ObjectId, input: ClientInput) -> Vec<Out> {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
        self.log.take()
    }

    fn leftover(&mut self, input: LeftoverInput) -> Vec<Out> {
        self.input(A, ClientInput::Leftover(input))
    }

    fn advance(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.world.tick();
        }
    }

    fn macros(&self) -> Vec<(i32, String)> {
        self.world
            .player(A)
            .unwrap()
            .data
            .profile
            .macros
            .iter()
            .map(|m| (m.id, m.name.clone()))
            .collect()
    }

    fn adena(&self) -> i64 {
        self.world.adena_of(A)
    }

    fn items(&self) -> Vec<(i32, i32, i64, String)> {
        let mut items: Vec<_> = self
            .world
            .player(A)
            .unwrap()
            .data
            .items
            .iter()
            .map(|i| (i.object_id.0, i.item_id, i.count, i.loc.clone()))
            .collect();
        items.sort_unstable();
        items
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn packets(out: &[Out], to: ObjectId) -> Vec<&[u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, bytes) if *id == to.0 => Some(bytes.as_slice()),
            _ => None,
        })
        .collect()
}

fn names_of(out: &[Out], to: ObjectId) -> Vec<String> {
    packets(out, to).into_iter().map(name_of).collect()
}

fn names(out: &[Out]) -> Vec<String> {
    names_of(out, A)
}

fn decoded<P: Decode>(out: &[Out], to: ObjectId, name: &str) -> Vec<P> {
    packets(out, to)
        .into_iter()
        .filter(|b| name_of(b) == name)
        .map(decode::<P>)
        .collect()
}

fn system_messages(out: &[Out]) -> Vec<SystemMessage> {
    decoded(out, A, "SYSTEM_MESSAGE")
}

fn system_ids(out: &[Out]) -> Vec<SystemMessageId> {
    system_messages(out)
        .into_iter()
        .filter(|m| m.id != SystemMessageId::S12)
        .map(|m| m.id)
        .collect()
}

fn texts(out: &[Out]) -> Vec<String> {
    system_messages(out)
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn macro_lists(out: &[Out]) -> Vec<MacroList> {
    decoded(out, A, "MACRO_LIST")
}

fn command(kind: u8, d1: i32, d2: u8, text: &str) -> MacroCommand {
    MacroCommand {
        entry: 1,
        kind,
        d1,
        d2,
        command: text.into(),
    }
}

fn make_macro(id: i32, name: &str) -> RequestMakeMacro {
    RequestMakeMacro {
        id,
        name: name.into(),
        descr: "desc".into(),
        acronym: "AC".into(),
        icon: 3,
        commands: vec![command(1, 1011, 0, ""), command(3, 0, 0, "/sit")],
    }
}

// ---- RequestMakeMacro / RequestDeleteMacro -----------------------------------------------------

/// `MacroList.registerMacro`: id 0 gets 1000, the next 1001; each save sends the whole list one
/// packet per macro, the revision up by one (the enter-world list is revision 2).
#[test]
fn a_new_macro_gets_an_id_from_1000_and_the_list_goes_out() {
    let mut s = Scene::new("make");
    s.enter(player(A, vec![]));
    let out = s.leftover(LeftoverInput::MakeMacro(make_macro(0, "Heal")));
    assert_eq!(names(&out), ["MACRO_LIST"]);
    let list = &macro_lists(&out)[0];
    assert_eq!((list.revision, list.count), (3, 1));
    let entry = list.macro_entry.as_ref().unwrap();
    assert_eq!((entry.id, entry.name.as_str(), entry.icon), (1000, "Heal", 3));
    assert_eq!(entry.commands.len(), 2);
    assert_eq!((entry.commands[0].kind, entry.commands[0].d1), (1, 1011));
    assert_eq!(entry.commands[1].cmd, "/sit");

    let out = s.leftover(LeftoverInput::MakeMacro(make_macro(0, "Buff")));
    // Two macros: two packets, both revision 4, both with the count 2.
    let lists = macro_lists(&out);
    assert_eq!(lists.len(), 2);
    assert!(lists.iter().all(|l| l.revision == 4 && l.count == 2));
    assert_eq!(s.macros(), [(1000, "Heal".to_owned()), (1001, "Buff".to_owned())]);
}

/// The type byte outside 1 to 6 is type 0 (`MacroType.values()[...]`).
#[test]
fn a_macro_command_type_out_of_range_is_none() {
    let mut s = Scene::new("make_type");
    s.enter(player(A, vec![]));
    let mut request = make_macro(0, "Odd");
    request.commands = vec![command(9, 1, 2, ""), command(6, 1, 2, "")];
    s.leftover(LeftoverInput::MakeMacro(request));
    let kinds: Vec<i32> = s.world.player(A).unwrap().data.profile.macros[0]
        .commands
        .iter()
        .map(|c| c.kind)
        .collect();
    assert_eq!(kinds, [0, 6]);
}

/// A macro saved again with its id replaces the old one in place.
#[test]
fn saving_a_macro_with_its_id_replaces_it_in_place() {
    let mut s = Scene::new("make_replace");
    s.enter(player(A, vec![]));
    s.leftover(LeftoverInput::MakeMacro(make_macro(0, "One")));
    s.leftover(LeftoverInput::MakeMacro(make_macro(0, "Two")));
    let out = s.leftover(LeftoverInput::MakeMacro(make_macro(1000, "Uno")));
    assert_eq!(s.macros(), [(1000, "Uno".to_owned()), (1001, "Two".to_owned())]);
    assert_eq!(macro_lists(&out).len(), 2);
    // The macro keeps its own name: the check for a taken name skips the same id.
    let out = s.leftover(LeftoverInput::MakeMacro(make_macro(1000, "UNO")));
    assert_eq!(names(&out), ["MACRO_LIST", "MACRO_LIST"]);
}

/// The id counter skips ids that are taken (`while (_macroses.containsKey(...))`).
#[test]
fn a_new_id_skips_the_taken_ones() {
    let mut s = Scene::new("make_skip");
    s.enter(player(A, vec![]));
    s.leftover(LeftoverInput::MakeMacro(make_macro(1000, "Taken")));
    s.leftover(LeftoverInput::MakeMacro(make_macro(0, "Next")));
    assert_eq!(
        s.macros(),
        [(1000, "Taken".to_owned()), (1001, "Next".to_owned())]
    );
}

/// The refusals of `RequestMakeMacro.runImpl`, in Java's order and with its messages.
#[test]
fn the_macro_refusals() {
    let mut s = Scene::new("make_refuse");
    s.enter(player(A, vec![]));

    // Enter the name of the macro.
    let out = s.leftover(LeftoverInput::MakeMacro(make_macro(0, "")));
    assert_eq!(system_ids(&out), [SystemMessageId::EnterTheNameOfTheMacro]);

    // Invalid macro: the commands are over 255 characters together.
    let mut request = make_macro(0, "Long");
    request.commands = vec![
        command(3, 0, 0, &"a".repeat(128)),
        command(3, 0, 0, &"b".repeat(128)),
    ];
    let out = s.leftover(LeftoverInput::MakeMacro(request));
    assert_eq!(
        system_ids(&out),
        [SystemMessageId::InvalidMacroReferToTheHelpFileForInstructions]
    );

    // 255 is fine.
    let mut request = make_macro(0, "Fits");
    request.commands = vec![command(3, 0, 0, &"a".repeat(255))];
    let out = s.leftover(LeftoverInput::MakeMacro(request));
    assert_eq!(names(&out), ["MACRO_LIST"]);

    // The description is over 32 characters.
    let mut request = make_macro(0, "Desc");
    request.descr = "d".repeat(33);
    let out = s.leftover(LeftoverInput::MakeMacro(request));
    assert_eq!(
        system_ids(&out),
        [SystemMessageId::MacroDescriptionsMayContainUpTo32Characters]
    );

    // NOTE(java-quirk): a name that is taken says "Enter the name of the macro" too, and the
    // comparison ignores the case.
    let out = s.leftover(LeftoverInput::MakeMacro(make_macro(0, "FITS")));
    assert_eq!(system_ids(&out), [SystemMessageId::EnterTheNameOfTheMacro]);
    assert_eq!(s.macros().len(), 1);
}

/// NOTE(java-quirk): `size() > 48` lets a character with 48 macros save a 49th, and refuses the 50th.
#[test]
fn the_macro_limit_is_one_more_than_48() {
    let mut s = Scene::new("make_limit");
    s.enter(player(A, vec![]));
    for i in 0..49 {
        let out = s.leftover(LeftoverInput::MakeMacro(make_macro(0, &format!("m{i}"))));
        assert!(names(&out).iter().all(|n| n == "MACRO_LIST"), "macro {i}");
    }
    assert_eq!(s.macros().len(), 49);
    let out = s.leftover(LeftoverInput::MakeMacro(make_macro(0, "one too many")));
    assert_eq!(system_ids(&out), [SystemMessageId::YouMayCreateUpTo48Macros]);
    assert_eq!(s.macros().len(), 49);
}

/// `MacroList.deleteMacro`: the macro goes, its shortcuts go (the list is sent again for each),
/// and the macro list follows with a higher revision. An empty list is one packet without a macro.
#[test]
fn deleting_a_macro_removes_its_shortcuts_and_resends_the_list() {
    let mut s = Scene::new("delete");
    let mut data = player(A, vec![]);
    data.shortcuts = vec![
        ShortcutData {
            slot: 0,
            page: 0,
            kind: 4,
            shortcut_id: 1000,
            level: 0,
        },
        ShortcutData {
            slot: 1,
            page: 0,
            kind: 4,
            shortcut_id: 1001,
            level: 0,
        },
        ShortcutData {
            slot: 2,
            page: 1,
            kind: 4,
            shortcut_id: 1000,
            level: 0,
        },
    ];
    s.enter(data);
    s.leftover(LeftoverInput::MakeMacro(make_macro(0, "One")));
    s.leftover(LeftoverInput::MakeMacro(make_macro(0, "Two")));
    let out = s.leftover(LeftoverInput::DeleteMacro { id: 1000 });
    assert_eq!(
        names(&out),
        ["SHORT_CUT_INIT", "SHORT_CUT_INIT", "MACRO_LIST"],
        "one list for each shortcut of the macro, then the macro list"
    );
    let lists = macro_lists(&out);
    assert_eq!((lists[0].revision, lists[0].count), (5, 1));
    assert_eq!(lists[0].macro_entry.as_ref().unwrap().id, 1001);
    assert_eq!(
        s.world
            .player(A)
            .unwrap()
            .data
            .shortcuts
            .iter()
            .map(|x| x.shortcut_id)
            .collect::<Vec<_>>(),
        [1001]
    );

    // The last one: a single packet without a macro.
    let out = s.leftover(LeftoverInput::DeleteMacro { id: 1001 });
    let lists = macro_lists(&out);
    assert_eq!(lists.len(), 1);
    assert_eq!((lists[0].count, lists[0].macro_entry.is_none()), (0, true));
    assert!(s.macros().is_empty());

    // An id that is not there still sends the list (Java does not check).
    let out = s.leftover(LeftoverInput::DeleteMacro { id: 5 });
    assert_eq!(macro_lists(&out).len(), 1);
}

// ---- RequestCrystallizeItem ---------------------------------------------------------------------

fn crystallize(object_id: i32, count: i64) -> LeftoverInput {
    LeftoverInput::Crystallize(RequestCrystallizeItem { object_id, count })
}

fn crystal_player(items: Vec<ItemData>, skill_level: i32) -> PlayerData {
    let mut data = player(A, items);
    if skill_level > 0 {
        data.skills = vec![SkillData {
            skill_id: 248,
            skill_level,
        }];
    }
    data
}

/// Crystallizing: the item goes, the crystals come, the messages in Java's order.
#[test]
fn crystallizing_an_item_gives_crystals() {
    let mut s = Scene::new("crystal");
    s.enter(crystal_player(vec![inv(100, B_SWORD, 1)], 3));
    let out = s.leftover(crystallize(100, 1));
    // The destroyed item at once, the load of the new crystals, two messages, the user info. The
    // update for the crystals waits 100 ms (`Player.sendInventoryUpdate`).
    assert_eq!(
        names(&out),
        [
            "INVENTORY_UPDATE",
            "STATUS_UPDATE",
            "SYSTEM_MESSAGE",
            "SYSTEM_MESSAGE",
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO"
        ]
    );
    s.advance(3);
    assert_eq!(names(&s.log.take()), ["INVENTORY_UPDATE"]);
    let items = s.items();
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].1, items[0].2), (CRYSTAL_B, 100));
    let messages = system_messages(&out);
    assert_eq!(messages[0].id, SystemMessageId::S1HasBeenCrystallized);
    assert_eq!(messages[0].params, [SystemMessageParam::ItemName(B_SWORD)]);
    assert_eq!(messages[1].id, SystemMessageId::YouHaveEarnedS2S1S);
    assert_eq!(
        messages[1].params,
        [
            SystemMessageParam::ItemName(CRYSTAL_B),
            SystemMessageParam::LongNumber(100)
        ]
    );
    // The row of the item is deleted at once.
    assert!(out.iter().any(|o| matches!(o, Out::Persist(PersistCommand::ItemChanges { removed, .. }) if removed == &[ObjectId(100)])));
}

/// The crystal count grows with the enchant level (`Item.getCrystalCount()`).
#[test]
fn an_enchanted_item_gives_more_crystals() {
    let mut s = Scene::new("crystal_enchant");
    let mut sword = inv(100, B_SWORD, 1);
    sword.enchant_level = 5;
    s.enter(crystal_player(vec![sword], 3));
    let expected = i64::from(
        ItemTemplates::from_xml(ITEMS)
            .unwrap()
            .get(B_SWORD)
            .unwrap()
            .crystal_count_at(5),
    );
    assert!(expected > 100);
    s.leftover(crystallize(100, 1));
    assert_eq!(s.items()[0].2, expected);
}

/// A worn item comes off first: the update for the unequip, then "has been disarmed".
#[test]
fn a_worn_item_is_taken_off_first() {
    let mut s = Scene::new("crystal_worn");
    s.enter(crystal_player(vec![item(100, B_SWORD, 1, "PAPERDOLL", 5)], 3));
    let out = s.leftover(crystallize(100, 1));
    // The unequip update, "disarmed", the destroy update, the load of the crystals ...
    let got: Vec<String> = names(&out)
        .into_iter()
        .filter(|n| n != "ETC_STATUS_UPDATE")
        .collect();
    assert_eq!(
        &got[..3],
        ["INVENTORY_UPDATE", "SYSTEM_MESSAGE", "INVENTORY_UPDATE"]
    );
    assert_eq!(system_messages(&out)[0].id, SystemMessageId::S1HasBeenDisarmed);
    assert_eq!(s.items().len(), 1);
}

/// Without the skill: the message and `ActionFailed`. An A-grade item needs level 4 or more.
#[test]
fn the_crystallize_skill_level_is_checked() {
    let mut s = Scene::new("crystal_skill");
    s.enter(crystal_player(
        vec![inv(100, B_SWORD, 1), inv(101, A_SWORD, 1)],
        0,
    ));
    let out = s.leftover(crystallize(100, 1));
    assert_eq!(names(&out), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert_eq!(
        system_ids(&out),
        [SystemMessageId::YouMayNotCrystallizeThisItemYourCrystallizationSkillLevelIsTooLow]
    );
    assert_eq!(s.items().len(), 2);

    let mut s = Scene::new("crystal_skill_a");
    s.enter(crystal_player(vec![inv(101, A_SWORD, 1)], 3));
    let out = s.leftover(crystallize(101, 1));
    assert_eq!(names(&out), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert_eq!(s.items().len(), 1);

    let mut s = Scene::new("crystal_skill_a4");
    s.enter(crystal_player(vec![inv(101, A_SWORD, 1)], 4));
    s.leftover(crystallize(101, 1));
    let items = s.items();
    assert_eq!((items[0].1, items[0].2), (CRYSTAL_A, 300));
}

/// The refusals: an item that is not there, a hero item, a plain item, a shadow item, a count of 0
/// (an illegal action), a bound count above the stack.
#[test]
fn the_crystallize_refusals() {
    let mut shadow = inv(103, SHADOW_SWORD, 1);
    shadow.mana_left = 600;
    let mut s = Scene::new("crystal_refuse");
    s.enter(crystal_player(
        vec![
            inv(100, HERO_SWORD, 1),
            inv(101, PLAIN_SWORD, 1),
            inv(102, B_SWORD, 1),
            shadow,
        ],
        5,
    ));
    // Not in the inventory, and a hero item: ActionFailed.
    assert_eq!(names(&s.leftover(crystallize(999, 1))), ["ACTION_FAIL"]);
    assert_eq!(names(&s.leftover(crystallize(100, 1))), ["ACTION_FAIL"]);
    // Not crystallizable and a shadow item: nothing at all.
    assert!(s.leftover(crystallize(101, 1)).is_empty());
    assert!(s.leftover(crystallize(103, 1)).is_empty());
    assert_eq!(s.items().len(), 4);
    // A count below 1 is an illegal action.
    let out = s.leftover(crystallize(102, 0));
    assert_eq!(texts(&out), [KICK]);
    assert_eq!(s.items().len(), 4);
    // A count over the stack is cut to it.
    s.leftover(crystallize(102, 50));
    assert!(s.items().iter().all(|i| i.0 != 102));
}

// ---- RequestPreviewItem -------------------------------------------------------------------------

fn preview(list_id: i32, items: &[i32]) -> LeftoverInput {
    LeftoverInput::PreviewItem(RequestPreviewItem {
        unknown: 0,
        list_id,
        items: Some(items.to_vec()),
    })
}

/// One player with 1000 adena, a trader 50 away that it targeted and talked to.
fn wear_scene(tag: &str) -> Scene {
    let mut s = Scene::new(tag);
    s.enter(player(A, vec![inv(100, ADENA, 1000)]));
    let npc = s.world.spawn_npc_at(30001, X + 50, Y, Z, 0).unwrap();
    s.input(
        A,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    s
}

/// The try-on: `WearPrice` (10) per item, `ShopPreviewInfo` with the items by slot, and after
/// `WearDelay` (5 s) the message and the user info.
#[test]
fn trying_items_on_costs_adena_and_ends_after_five_seconds() {
    let mut s = wear_scene("preview");
    let out = s.leftover(preview(100, &[SWORD, HAT]));
    assert_eq!(s.adena(), 980);
    let info: ShopPreviewInfo = decoded(&out, A, "SHOP_PREVIEW_INFO").remove(0);
    // RHAND (twice on the wire) and HEAD; Java's order UNDER REAR LEAR NECK RFINGER LFINGER HEAD RHAND ...
    assert_eq!(info.item_ids[6], HAT);
    assert_eq!(info.item_ids[7], SWORD);
    assert_eq!(info.item_ids[14], SWORD);
    assert_eq!(info.item_ids.iter().filter(|id| **id != 0).count(), 3);
    // The adena was taken: the message comes at once, the update 100 ms later.
    assert!(system_ids(&out).contains(&SystemMessageId::S1AdenaDisappeared));

    // 99 ticks (4.95 s): nothing yet. One more: the end.
    s.advance(99);
    assert!(names(&s.log.take()).iter().all(|n| n != "USER_INFO"));
    s.advance(1);
    let out = s.log.take();
    assert_eq!(
        system_ids(&out),
        [SystemMessageId::YouAreNoLongerTryingOnEquipment2]
    );
    assert!(names(&out).contains(&"USER_INFO".to_owned()));
}

/// Two items for one slot: refused with the message, nothing is charged.
#[test]
fn two_items_for_one_slot_are_refused() {
    let mut s = wear_scene("preview_slot");
    let out = s.leftover(preview(100, &[SWORD, SWORD_2]));
    assert_eq!(
        system_ids(&out),
        [SystemMessageId::YouCanNotTryThoseItemsOnAtTheSameTime]
    );
    assert_eq!(s.adena(), 1000);
}

/// Without the adena: the message twice (Java's `reduceAdena(..., true)` sends it, and so does the
/// handler), and no try-on.
#[test]
fn not_enough_adena_says_so_twice() {
    let mut s = Scene::new("preview_poor");
    s.enter(player(A, vec![inv(100, ADENA, 5)]));
    let npc = s.world.spawn_npc_at(30001, X + 50, Y, Z, 0).unwrap();
    s.input(
        A,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    let out = s.leftover(preview(100, &[SWORD]));
    assert_eq!(
        system_ids(&out),
        [
            SystemMessageId::YouDoNotHaveEnoughAdena,
            SystemMessageId::YouDoNotHaveEnoughAdena
        ]
    );
    assert!(!names(&out).contains(&"SHOP_PREVIEW_INFO".to_owned()));
    assert_eq!(s.adena(), 5);
}

/// A Kamael does not try on heavy armor: that item is skipped and the others still work.
#[test]
fn a_kamael_skips_heavy_armor() {
    let mut s = Scene::new("preview_kamael");
    let mut data = player(A, vec![inv(100, ADENA, 1000)]);
    data.race = 5;
    s.enter(data);
    let npc = s.world.spawn_npc_at(30001, X + 50, Y, Z, 0).unwrap();
    s.input(
        A,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    let out = s.leftover(preview(100, &[CUIRASS, HAT]));
    assert_eq!(s.adena(), 990, "only the hat is charged");
    let info: ShopPreviewInfo = decoded(&out, A, "SHOP_PREVIEW_INFO").remove(0);
    assert_eq!(info.item_ids.iter().filter(|id| **id != 0).count(), 1);
}

/// Everything that makes Java return or fail: no merchant target, too far, an empty or too long
/// list, a list that does not exist, an item that is not in the list.
#[test]
fn the_wear_shop_refusals() {
    // No target: nothing happens at all.
    let mut s = Scene::new("preview_notarget");
    s.enter(player(A, vec![inv(100, ADENA, 1000)]));
    assert!(s.leftover(preview(100, &[SWORD])).is_empty());
    assert_eq!(s.adena(), 1000);

    // A villager is no merchant.
    let villager = s.world.spawn_npc_at(30003, X + 50, Y, Z, 0).unwrap();
    s.input(
        A,
        ClientInput::Action {
            object_id: villager.0,
            action_id: 0,
        },
    );
    s.log.take();
    assert!(s.leftover(preview(100, &[SWORD])).is_empty());

    let mut s = wear_scene("preview_refuse");
    // An empty list and a list id of the custom shops: ActionFailed.
    assert_eq!(names(&s.leftover(preview(100, &[]))), ["ACTION_FAIL"]);
    assert_eq!(names(&s.leftover(preview(4_000_000, &[SWORD]))), ["ACTION_FAIL"]);
    // More than 100 items: the packet has no list.
    let out = s.leftover(LeftoverInput::PreviewItem(RequestPreviewItem {
        unknown: 0,
        list_id: 100,
        items: None,
    }));
    assert!(out.is_empty());
    // A list that does not exist, and an item that is not in it: illegal actions.
    assert_eq!(texts(&s.leftover(preview(555, &[SWORD]))), [KICK]);
    assert_eq!(texts(&s.leftover(preview(100, &[B_SWORD]))), [KICK]);
    assert_eq!(s.adena(), 1000);
}

/// A merchant too far away (250 or more): nothing.
#[test]
fn a_merchant_out_of_reach_is_refused() {
    let mut s = Scene::new("preview_far");
    s.enter(player(A, vec![inv(100, ADENA, 1000)]));
    let npc = s.world.spawn_npc_at(30001, X + 300, Y, Z, 0).unwrap();
    s.input(
        A,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    assert!(s.leftover(preview(100, &[SWORD])).is_empty());
    assert_eq!(s.adena(), 1000);
}

// ---- RequestSaveKeyMapping / RequestKeyMapping --------------------------------------------------

/// The bytes become `UI_KEY_MAPPING`: signed decimals, each followed by a tab; the next
/// `RequestKeyMapping` answers with them. The variable is marked for the next store.
#[test]
fn the_key_mapping_is_stored_as_a_player_variable() {
    let mut s = Scene::new("keymap");
    s.enter(player(A, vec![]));
    let out = s.leftover(LeftoverInput::SaveKeyMapping(Some(vec![1, 0xFF, 3])));
    assert!(out.is_empty());
    let profile = &s.world.player(A).unwrap().data.profile;
    assert_eq!(
        profile.variables.get("UI_KEY_MAPPING").map(String::as_str),
        Some("1\t-1\t3\t")
    );
    assert!(profile.variables_changed.contains("UI_KEY_MAPPING"));

    let out = s.input(A, ClientInput::KeyMapping);
    let setting: l2_protocol_game::server::enterworld::ExUiSetting =
        decoded(&out, A, "EX_UI_SETTING").remove(0);
    assert_eq!(setting.ui_key_mapping, [1, 0xFF, 3]);

    // No bytes: nothing changes.
    s.leftover(LeftoverInput::SaveKeyMapping(None));
    assert_eq!(
        s.world.player(A).unwrap().data.profile.variables["UI_KEY_MAPPING"],
        "1\t-1\t3\t"
    );
}

/// `StoreCharUiSettings = False`: Java stores nothing.
#[test]
fn the_key_mapping_needs_the_config() {
    let mut s = Scene::with(
        "keymap_off",
        WorldSettings {
            store_ui_settings: false,
            ..WorldSettings::default()
        },
    );
    s.enter(player(A, vec![]));
    s.leftover(LeftoverInput::SaveKeyMapping(Some(vec![1])));
    // Only the key mapping matters: the Nevit daily reset may add `hunting_time` depending on the
    // wall clock at enter.
    assert!(
        !s.world
            .player(A)
            .unwrap()
            .data
            .profile
            .variables
            .contains_key(l2_world::leftover::UI_KEY_MAPPING)
    );
}

// ---- RequestSaveInventoryOrder ------------------------------------------------------------------

/// Items of the inventory get the slot the client says; a worn item and an unknown id are left
/// alone. The item list is compared with the store, so `loc_data` is written with the next store.
#[test]
fn the_inventory_order_sets_the_slot_of_inventory_items() {
    let mut s = Scene::new("order");
    s.enter(player(
        A,
        vec![
            inv(100, ADENA, 10),
            inv(101, HAT, 1),
            item(102, SWORD, 1, "PAPERDOLL", 5),
        ],
    ));
    s.leftover(LeftoverInput::SaveInventoryOrder(vec![
        (100, 7),
        (101, 3),
        (102, 9),
        (999, 1),
    ]));
    let slots: Vec<(i32, i32)> = s
        .world
        .player(A)
        .unwrap()
        .data
        .items
        .iter()
        .map(|i| (i.object_id.0, i.loc_data))
        .collect();
    assert_eq!(slots, [(100, 7), (101, 3), (102, 5)]);
}

// ---- the small requests -------------------------------------------------------------------------

/// `new ShowMiniMap(1665)`.
#[test]
fn the_mini_map_is_map_1665() {
    let mut s = Scene::new("minimap");
    s.enter(player(A, vec![]));
    let out = s.leftover(LeftoverInput::ShowMiniMap);
    assert_eq!(names(&out), ["SHOW_MINIMAP"]);
    let map: ShowMiniMap = decoded(&out, A, "SHOW_MINIMAP").remove(0);
    assert_eq!(map.map_id, 1665);
}

/// `player.sendSkillList()` waits 300 ms, like everywhere else.
#[test]
fn the_skill_list_request_sends_the_list_after_300_ms() {
    let mut s = Scene::new("skilllist");
    s.enter(player(A, vec![]));
    assert!(s.leftover(LeftoverInput::SkillList).is_empty());
    s.advance(5);
    assert!(s.log.take().is_empty());
    s.advance(1);
    assert_eq!(names(&s.log.take()), ["SKILL_LIST"]);
}

/// `RequestRecordInfo`: the player's own info and the info of everything it sees.
#[test]
fn record_info_sends_the_info_of_everything_in_view() {
    let mut s = Scene::new("record");
    s.enter(player(A, vec![]));
    let mut other = player(B, vec![]);
    other.x += 20;
    s.enter(other);
    let npc = s.world.spawn_npc_at(30003, X + 40, Y, Z, 0).unwrap();
    s.advance(20);
    s.log.take();
    let out = s.leftover(LeftoverInput::RecordInfo);
    let got = names(&out);
    assert_eq!(got[0], "USER_INFO");
    assert_eq!(got[1], "EX_BR_EXTRA_USER_INFO");
    assert!(got.contains(&"CHAR_INFO".to_owned()), "the other player: {got:?}");
    assert!(got.contains(&"NPC_INFO".to_owned()), "the NPC {npc:?}: {got:?}");
    // Nothing goes to the other player.
    assert!(packets(&out, B).is_empty());
}

/// `StartRotating` / `FinishRotating`: the others see it, the player is not sent its own packet;
/// the heading follows the finish. `KeyboardMovement = False` ignores both.
#[test]
fn rotating_with_the_keyboard_is_broadcast() {
    let mut s = Scene::new("rotate");
    s.enter(player(A, vec![]));
    let mut other = player(B, vec![]);
    other.x += 20;
    s.enter(other);

    let out = s.leftover(LeftoverInput::StartRotating {
        degree: 16384,
        side: 1,
    });
    assert!(packets(&out, A).is_empty());
    let start: StartRotation = decoded(&out, B, "START_ROTATING").remove(0);
    assert_eq!(
        start,
        StartRotation {
            object_id: 1,
            degree: 16384,
            side: 1,
            speed: 0
        }
    );

    let out = s.leftover(LeftoverInput::FinishRotating { degree: 32768 });
    let stop: StopRotation = decoded(&out, B, "FINISH_ROTATING").remove(0);
    assert_eq!(
        stop,
        StopRotation {
            object_id: 1,
            degree: 32768,
            speed: 0
        }
    );
    assert_eq!(s.world.player(A).unwrap().core.heading, 32768);

    let mut off = Scene::with(
        "rotate_off",
        WorldSettings {
            keyboard_movement: false,
            ..WorldSettings::default()
        },
    );
    off.enter(player(A, vec![]));
    let mut other = player(B, vec![]);
    other.x += 20;
    off.enter(other);
    assert!(
        off.leftover(LeftoverInput::StartRotating { degree: 1, side: 1 })
            .is_empty()
    );
    assert!(
        off.leftover(LeftoverInput::FinishRotating { degree: 5 })
            .is_empty()
    );
    assert_eq!(off.world.player(A).unwrap().core.heading, 0);
}

// ---- RequestDispel ------------------------------------------------------------------------------

fn buffed(skills: &[i32]) -> Scene {
    let mut s = Scene::new("dispel");
    s.enter(player(A, vec![]));
    for id in skills {
        s.world.apply_effects(A, A, SkillKey::new(*id, 1));
    }
    s.log.take();
    s
}

fn dispel(skill_id: i32) -> LeftoverInput {
    LeftoverInput::Dispel(RequestDispel {
        object_id: A.0,
        skill_id,
        skill_level: 1,
    })
}

fn skills_on(s: &Scene) -> usize {
    s.world
        .effect_list(A)
        .map_or(0, |l| l.queue(ListKind::Buffs).len())
}

/// A buff comes off: "The effect of $s1 has been removed."
#[test]
fn a_buff_can_be_taken_off() {
    let mut s = buffed(&[9001]);
    assert_eq!(skills_on(&s), 1);
    let out = s.leftover(dispel(9001));
    assert_eq!(skills_on(&s), 0);
    assert_eq!(system_ids(&out), [SystemMessageId::TheEffectOfS1HasBeenRemoved]);
}

/// Debuffs, skills that stay after death, skills that cannot be dispelled, unknown skills and
/// bad ids stay; a dance only with `DanceCancelBuff`.
#[test]
fn what_cannot_be_dispelled() {
    let mut s = buffed(&[9040, 9060, 9070, 9020]);
    let before = skills_on(&s);
    assert!(before >= 2);
    s.leftover(dispel(9040));
    s.leftover(dispel(9060));
    s.leftover(dispel(9070));
    s.leftover(dispel(77777));
    s.leftover(LeftoverInput::Dispel(RequestDispel {
        object_id: A.0,
        skill_id: 9001,
        skill_level: 0,
    }));
    // The dance: refused by default.
    s.leftover(dispel(9020));
    assert_eq!(skills_on(&s), before);

    let mut s = Scene::with(
        "dispel_dance",
        WorldSettings {
            leftover: LeftoverSettings {
                dance_cancel_buff: true,
                ..LeftoverSettings::default()
            },
            ..WorldSettings::default()
        },
    );
    s.enter(player(A, vec![]));
    s.world.apply_effects(A, A, SkillKey::new(9020, 1));
    let dances = s
        .world
        .effect_list(A)
        .map_or(0, |l| l.queue(ListKind::Dances).len());
    assert_eq!(dances, 1);
    s.leftover(dispel(9020));
    assert_eq!(
        s.world
            .effect_list(A)
            .map_or(0, |l| l.queue(ListKind::Dances).len()),
        0
    );
}

/// Another object id than the player's (and not its summon) does nothing.
#[test]
fn dispel_of_a_stranger_does_nothing() {
    let mut s = buffed(&[9001]);
    s.leftover(LeftoverInput::Dispel(RequestDispel {
        object_id: 12345,
        skill_id: 9001,
        skill_level: 1,
    }));
    assert_eq!(skills_on(&s), 1);
}

// ---- RequestExMagicSkillUseGround ---------------------------------------------------------------

fn ground(skill_id: i32) -> LeftoverInput {
    LeftoverInput::MagicSkillUseGround(RequestExMagicSkillUseGround {
        x: X,
        y: Y + 100,
        z: Z,
        skill_id,
        ctrl_pressed: false,
        shift_pressed: false,
    })
}

/// A skill the player does not know: `ActionFailed`.
#[test]
fn a_ground_skill_the_player_does_not_know_fails() {
    let mut s = Scene::new("ground_unknown");
    s.enter(player(A, vec![]));
    assert_eq!(names(&s.leftover(ground(9001))), ["ACTION_FAIL"]);
}

/// A known skill: the ground point is kept, the player turns to it and the others get a
/// `ValidateLocation` with the new heading.
#[test]
fn a_ground_skill_turns_the_player_and_remembers_the_point() {
    let mut s = Scene::new("ground");
    let mut data = player(A, vec![]);
    data.skills = vec![SkillData {
        skill_id: 9001,
        skill_level: 1,
    }];
    s.enter(data);
    let mut other = player(B, vec![]);
    other.x += 20;
    s.enter(other);
    let out = s.leftover(ground(9001));
    // Straight "up" the y axis: 90 degrees, 16384 in the client's units.
    let heading = s.world.player(A).unwrap().core.heading;
    assert!((16380..=16390).contains(&heading), "{heading}");
    let packet: ValidateLocation = decoded(&out, B, "VALIDATE_LOCATION").remove(0);
    assert_eq!((packet.object_id, packet.x, packet.y, packet.z), (1, X, Y, Z));
    assert_eq!(packet.heading, heading);
    assert!(!names_of(&out, A).contains(&"VALIDATE_LOCATION".to_owned()));
}

// ---- RequestExRqItemLink ------------------------------------------------------------------------

fn say(text: &str) -> ClientInput {
    ClientInput::Say {
        chat_type: 0,
        text: text.into(),
        target: None,
    }
}

/// An item shown in the chat can be looked up by its object id, by anybody; an item that was not
/// shown (or does not exist) gets no answer.
#[test]
fn an_item_link_answers_only_for_published_items() {
    let mut s = Scene::new("itemlink");
    let mut sword = inv(100, B_SWORD, 1);
    sword.enchant_level = 3;
    s.enter(player(A, vec![sword, inv(101, HAT, 1)]));
    let mut other = player(B, vec![]);
    other.x += 20;
    s.enter(other);

    // Not published yet.
    let out = s.input(
        B,
        ClientInput::Leftover(LeftoverInput::ExRqItemLink { object_id: 100 }),
    );
    assert!(packets(&out, B).is_empty());

    // A shows the sword in the chat: `\b` ... `ID=100` ... `\b`.
    s.input(A, say("\u{8}1\tID=100 ENCHANT=3\u{8}"));
    let out = s.input(
        B,
        ClientInput::Leftover(LeftoverInput::ExRqItemLink { object_id: 100 }),
    );
    assert_eq!(names_of(&out, B), ["EX_RP_ITEM_LINK"]);
    let link: ExRpItemLink = decoded(&out, B, "EX_RP_ITEM_LINK").remove(0);
    assert_eq!(
        (link.item.object_id, link.item.item_id, link.item.count),
        (100, B_SWORD, 1)
    );
    assert_eq!(link.item.enchant, 3);

    // The hat was not shown, and an id that does not exist is nothing.
    let out = s.input(
        B,
        ClientInput::Leftover(LeftoverInput::ExRqItemLink { object_id: 101 }),
    );
    assert!(packets(&out, B).is_empty());
    let out = s.input(
        B,
        ClientInput::Leftover(LeftoverInput::ExRqItemLink { object_id: 4242 }),
    );
    assert!(packets(&out, B).is_empty());
}

/// Java publishes each item as its link is read, so the links before a bad one stay published even
/// though the message is dropped.
#[test]
fn links_before_a_bad_one_stay_published() {
    let mut s = Scene::new("itemlink_bad");
    s.enter(player(A, vec![inv(100, B_SWORD, 1)]));
    let mut other = player(B, vec![]);
    other.x += 20;
    s.enter(other);
    // The second link names an item A does not own: the whole text is dropped.
    let out = s.input(A, say("\u{8}ID=100\u{8} \u{8}ID=999\u{8}"));
    assert!(packets(&out, B).is_empty(), "the message is not sent");
    let out = s.input(
        B,
        ClientInput::Leftover(LeftoverInput::ExRqItemLink { object_id: 100 }),
    );
    assert_eq!(names_of(&out, B), ["EX_RP_ITEM_LINK"]);
}
