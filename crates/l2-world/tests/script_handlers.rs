#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The handlers S-HAND-02 ported: the newbie support NPC (`SupportMagic`, `SupportMagicServitor`,
//! `GiveBlessing`), `BuyShadowItem`, `Wear`, `ReceivePremium`, `voice`, `RentPet`, `FindPvP`, the
//! voiced `.expoff`/`.expon`, the user commands `/unstuck` and `/mybirthday`, and the items
//! `CharmOfCourage`, `RollingDice` and `EventItem`. Expected behaviour comes from the Java
//! handlers named in each test (dist/game/data/scripts/handlers/...).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, BuyListOptions, BuyLists, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions,
    NpcTemplates, PlayerTemplates, SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    Dice, EtcStatusUpdate, NpcHtmlMessage, SetupGauge, ShopPreviewList, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Vec<u8>>>>);

impl Log {
    fn take(&self) -> Vec<Vec<u8>> {
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

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

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
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="10" type="Weapon" name="No-grade Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="price" val="100" /></item>
<item id="2" type="Weapon" name="C Sword"><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="price" val="500" /></item>
<item id="1835" type="EtcItem" name="Soulshot"><set name="is_stackable" val="true" /><set name="price" val="5" /></item>
<item id="4625" type="EtcItem" name="Dice"><set name="handler" val="RollingDice" /></item>
<item id="9201" type="EtcItem" name="Charm of Courage D"><set name="crystal_type" val="D" /><set name="handler" val="CharmOfCourage" /></item>
<item id="9202" type="EtcItem" name="Charm of Courage C"><set name="crystal_type" val="C" /><set name="handler" val="CharmOfCourage" /></item>
<item id="13787" type="EtcItem" name="Handy's Block Checker Bond"><set name="handler" val="EventItem" /><skills><skill id="9301" level="1" /></skills></item>
<item id="13799" type="EtcItem" name="Odd Event Item"><set name="handler" val="EventItem" /></item>
</list>"#;

/// The newbie buffs are plain buffs with a unique abnormal type each; 9301 is the Bond's skill.
fn skills_xml() -> String {
    // A real abnormal type each, so the buffs do not replace one another.
    let types = [
        ("4322", "SPEED_UP"),
        ("4323", "PD_UP"),
        ("5637", "MD_UP"),
        ("4324", "PA_UP"),
        ("4325", "MA_UP"),
        ("4326", "HP_REGEN_UP"),
        ("4327", "ATTACK_TIME_UP"),
        ("5632", "CRITICAL_PROB_UP"),
        ("4328", "MAX_HP_UP"),
        ("4329", "MAX_MP_UP"),
        ("4330", "CP_UP"),
        ("4331", "HIT_UP"),
        ("4338", "AVOID_UP"),
        ("5182", "PK_PROTECT"),
    ];
    let mut xml = String::from("<list>");
    for (id, abnormal) in types {
        xml.push_str(&format!(
            "<skill id=\"{id}\" levels=\"1\" name=\"Buff {id}\"><operateType>A2</operateType><targetType>ONE</targetType>             <abnormalLevel>1</abnormalLevel><abnormalTime>600</abnormalTime><abnormalType>{abnormal}</abnormalType>             <castRange>400</castRange><effectRange>900</effectRange><isMagic>1</isMagic>             <effects><effect name=\"Buff\"><mul stat=\"pDef\" val=\"1.01\"/></effect></effects></skill>"
        ));
    }
    xml.push_str("</list>");
    xml
}

const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="30001" type="Merchant" level="10" name="Trader" usingServerSideName="true">
  <race>FAIRY</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="300" hpRegen="2" mp="400.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="20" /><run ground="50" /></speed>
  </stats>
  <collision><radius normal="10"/><height normal="20"/></collision>
</npc>
<npc id="30827" type="Merchant" level="10" name="Pet Manager" usingServerSideName="true"/>
<npc id="30003" type="Folk" level="10" name="Villager" usingServerSideName="true">
  <race>FAIRY</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="300" hpRegen="2" mp="400.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="20" /><run ground="50" /></speed>
  </stats>
  <collision><radius normal="10"/><height normal="20"/></collision>
</npc>
</list>"#;

const BUY_LISTS: [(&str, &str); 1] = [(
    "0000100.xml",
    r#"<list><npcs><npc>30001</npc></npcs><item id="10" price="100" /><item id="2" price="500" /><item id="1835" price="5" /></list>"#,
)];

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
            &skills_xml(),
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x1000_0001);
const OTHER: ObjectId = ObjectId(0x1000_0002);

fn item(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".to_owned(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

fn player_data(id: ObjectId, level: i32, class_id: i32, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: format!("P{}", id.0 & 0xff),
        title: String::new(),
        class_id,
        base_class: class_id,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
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
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    me: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new(level: i32, class_id: i32, items: Vec<ItemData>) -> Self {
        Self::with(player_data(ME, level, class_id, items))
    }

    fn with(me_data: PlayerData) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        for page in [
            "default/SupportMagicHighLevel.htm",
            "default/SupportMagicLowLevel.htm",
            "default/SupportMagicNoSummon.htm",
            "default/SupportBlessingHighLevel.htm",
            "common/shadow_item-lowlevel.htm",
            "common/shadow_item_d.htm",
            "common/shadow_item_c.htm",
            "common/shadow_item_b.htm",
        ] {
            write(
                &format!("data/html/{page}"),
                &format!("<html><body>{page} %objectId%</body></html>"),
            );
        }
        let mut world = World::new(WorldRng::from_seed(5), Box::new(NullOutbox));
        let data = data();
        let items = Arc::clone(&data.items);
        world.set_static_data(Box::new(data));
        world.set_regen_settings(RegenSettings {
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        let rows: String = (1..=85)
            .map(|level| format!(r#"<experience level="{level}" tolevel="{}" />"#, level * 100_000))
            .collect();
        world.set_exp_data(
            Arc::new(
                l2_data::ExperienceTable::from_xml(&format!(
                    r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
                ))
                .unwrap(),
            ),
            Arc::new(l2_data::ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(l2_data::KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(l2_data::SkillTrees::from_xml("<list></list>").unwrap()),
        );
        world.set_flood_settings(FloodSettings::java_defaults());
        let lists = BuyLists::from_xml(&BUY_LISTS, &items, &BuyListOptions::default());
        world.set_buy_lists(
            Arc::new(lists),
            Arc::new(l2_data::MerchantPriceConfigs::from_xml(PRICE_CONFIGS).unwrap()),
        );
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(me_data.object_id.0).unwrap()),
            player: Box::new(me_data),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let mut scene = Self {
            world,
            me,
            _root: root,
        };
        scene.advance(1000);
        scene.me.take();
        scene
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    /// Spawns an NPC; what its appearance sent is not interesting.
    fn npc(&mut self, npc_id: i32, dx: i32) -> ObjectId {
        let npc = self.world.spawn_npc_at(npc_id, X + dx, Y, Z, 0).unwrap();
        self.me.take();
        npc
    }

    /// Runs the handler `Java BypassHandler.getHandler(command)` finds.
    fn bypass(&mut self, command: &str, npc: Option<ObjectId>) -> Option<bool> {
        let handler = BypassHandlers::default().get(command)?;
        Some(handler(&mut self.world, ME, command, npc))
    }

    fn user_command(&mut self, id: i32) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            input: ClientInput::UserCommand { id },
        });
    }

    fn say(&mut self, text: &str) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            input: ClientInput::Say {
                chat_type: 0,
                text: text.to_owned(),
                target: None,
            },
        });
    }

    fn use_item(&mut self, object_id: i32) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            input: ClientInput::UseItem {
                object_id,
                ctrl_pressed: false,
            },
        });
    }

    fn count_of(&self, item_id: i32) -> i64 {
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
}

const PRICE_CONFIGS: &str = r#"<merchantPriceConfig defaultPriceConfig="18">
    <priceConfig id="18" name="Neutral" baseTax="50" />
</merchantPriceConfig>"#;

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn names(out: &[Vec<u8>]) -> Vec<String> {
    out.iter().map(|b| name_of(b)).collect()
}

fn of<P: Decode>(out: &[Vec<u8>], opcode: ServerOpcode) -> Vec<P> {
    out.iter()
        .filter(|b| name_of(b) == opcode.name())
        .map(|b| decode::<P>(b))
        .collect()
}

fn messages(out: &[Vec<u8>]) -> Vec<SystemMessage> {
    of(out, ServerOpcode::SystemMessage)
}

fn html(out: &[Vec<u8>]) -> Vec<NpcHtmlMessage> {
    of(out, ServerOpcode::NpcHtmlMessage)
}

// ---------------------------------------------------------------------------------- SupportMagic

/// `SupportMagic.makeSupportMagic`: a fighter below level 40 gets the six fighter buffs and the
/// first haste (4327); the cubic skill is only for levels 16 to 34.
#[test]
fn support_magic_gives_a_fighter_its_buffs() {
    let mut s = Scene::new(20, 0, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("supportmagic", Some(npc)), Some(true));
    s.advance(2000);
    for skill in [4322, 4323, 5637, 4324, 4325, 4326, 4327] {
        assert!(s.world.is_affected_by_skill(ME, skill), "{skill}");
    }
    // The mage buffs are not given to a fighter, nor the second haste.
    for skill in [4328, 4329, 4330, 4331, 5632] {
        assert!(!s.world.is_affected_by_skill(ME, skill), "{skill}");
    }
}

/// A beginner mage (class 10) gets the seven mage buffs and no haste.
#[test]
fn support_magic_gives_a_mage_its_buffs() {
    let mut s = Scene::new(50, 10, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("supportmagic", Some(npc)), Some(true));
    s.advance(2000);
    for skill in [4322, 4323, 5637, 4328, 4329, 4330, 4331] {
        assert!(s.world.is_affected_by_skill(ME, skill), "{skill}");
    }
    for skill in [4324, 4327, 5632] {
        assert!(!s.world.is_affected_by_skill(ME, skill), "{skill}");
    }
}

/// From level 40 the fighter gets Haste 2 (5632) instead of 4327.
#[test]
fn support_magic_uses_the_second_haste_from_level_40() {
    let mut s = Scene::new(45, 0, vec![]);
    let npc = s.npc(30003, 50);
    s.bypass("supportmagic", Some(npc));
    s.advance(2000);
    assert!(s.world.is_affected_by_skill(ME, 5632));
    assert!(!s.world.is_affected_by_skill(ME, 4327));
}

/// Levels above 75 and below 6 are told to read the page; the third class gets the custom text.
#[test]
fn support_magic_pages_for_the_wrong_level() {
    let mut s = Scene::new(80, 0, vec![]);
    let npc = s.npc(30003, 50);
    s.bypass("supportmagic", Some(npc));
    let out = s.me.take();
    let pages = html(&out);
    assert_eq!(pages.len(), 1);
    assert!(pages[0].html.contains("SupportMagicHighLevel.htm"));
    assert!(pages[0].html.contains(&npc.0.to_string()));
    assert!(names(&out).contains(&"ACTION_FAIL".to_owned()));

    let mut s = Scene::new(5, 0, vec![]);
    let npc = s.npc(30003, 50);
    s.bypass("supportmagic", Some(npc));
    assert!(html(&s.me.take())[0].html.contains("SupportMagicLowLevel.htm"));

    // 112 is a third class.
    let mut s = Scene::new(70, 112, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("supportmagic", Some(npc)), Some(true));
    let out = s.me.take();
    assert!(html(&out).is_empty());
    let texts: Vec<String> = messages(&out)
        .iter()
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        ["Only adventurers who have not completed their 3rd class transfer may receive these buffs."]
    );
}

/// The servitor variant without a servitor shows `SupportMagicNoSummon.htm`.
#[test]
fn support_magic_servitor_needs_a_servitor() {
    let mut s = Scene::new(20, 0, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("supportmagicservitor", Some(npc)), Some(true));
    assert!(html(&s.me.take())[0].html.contains("SupportMagicNoSummon.htm"));
}

/// `SupportMagic` answers `false` when the target is not an NPC.
#[test]
fn support_magic_needs_an_npc() {
    let mut s = Scene::new(20, 0, vec![]);
    assert_eq!(s.bypass("supportmagic", None), Some(false));
    assert_eq!(s.bypass("supportmagic", Some(OTHER)), Some(false));
}

/// `SupportBlessing`: the Blessing of Protection under level 40 and the second class; the quirk is
/// that the handler answers `false` after the cast.
#[test]
fn give_blessing_casts_and_answers_false() {
    let mut s = Scene::new(20, 0, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("GiveBlessing", Some(npc)), Some(false));
    s.advance(2000);
    assert!(s.world.is_affected_by_skill(ME, 5182));
}

#[test]
fn give_blessing_is_refused_to_a_high_level() {
    let mut s = Scene::new(41, 0, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("GiveBlessing", Some(npc)), Some(true));
    assert!(
        html(&s.me.take())[0]
            .html
            .contains("SupportBlessingHighLevel.htm")
    );
    s.advance(2000);
    assert!(!s.world.is_affected_by_skill(ME, 5182));

    // The second class (88) at level 30 is refused too.
    let mut s = Scene::new(30, 88, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("GiveBlessing", Some(npc)), Some(true));
}

// ---------------------------------------------------------------------------------- BuyShadowItem

/// `BuyShadowItem`: the page by level (under 40, 40 to 45, 46 to 51, 52 and up), merchants only.
#[test]
fn buy_shadow_item_pages_by_level() {
    for (level, page) in [
        (39, "shadow_item-lowlevel.htm"),
        (40, "shadow_item_d.htm"),
        (45, "shadow_item_d.htm"),
        (46, "shadow_item_c.htm"),
        (51, "shadow_item_c.htm"),
        (52, "shadow_item_b.htm"),
    ] {
        let mut s = Scene::new(level, 0, vec![]);
        let npc = s.npc(30001, 50);
        assert_eq!(s.bypass("BuyShadowItem", Some(npc)), Some(true), "{level}");
        let pages = html(&s.me.take());
        assert_eq!(pages.len(), 1);
        assert!(pages[0].html.contains(page), "{level}: {}", pages[0].html);
        assert!(pages[0].html.contains(&npc.0.to_string()));
    }
    // A Folk is not a Merchant.
    let mut s = Scene::new(40, 0, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("BuyShadowItem", Some(npc)), Some(false));
}

// ---------------------------------------------------------------------------------- Wear

/// `Wear`: the preview list of the shop; items above the player's grade (the Expertise skill, 0
/// here) are left out and each costs `WearPrice`. The inventory is blocked for the click.
#[test]
fn wear_opens_the_preview_list() {
    let mut s = Scene::new(40, 0, vec![item(0x100, 57, 5000)]);
    let npc = s.npc(30001, 50);
    assert_eq!(s.bypass("Wear 100", Some(npc)), Some(true));
    let out = s.me.take();
    let lists: Vec<ShopPreviewList> = of(&out, ServerOpcode::ShopPreviewList);
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].money, 5000);
    assert_eq!(lists[0].list_id, 100);
    // The no-grade sword passes; the C sword and the soulshot (not equipable) do not.
    assert_eq!(lists[0].products.len(), 1);
    assert_eq!(lists[0].products[0].item_id, 10);
    assert_eq!(lists[0].products[0].body_part, 128);
    assert_eq!(lists[0].products[0].price, 10);
    assert!(s.world.player(ME).unwrap().shop.inventory_disabled);
}

#[test]
fn wear_answers_false_for_bad_input() {
    let mut s = Scene::new(40, 0, vec![]);
    let npc = s.npc(30001, 50);
    assert_eq!(s.bypass("Wear", Some(npc)), Some(false));
    assert_eq!(s.bypass("Wear x", Some(npc)), Some(false));
    // An unknown list: `ActionFailed`, and the handler still answers `true`.
    assert_eq!(s.bypass("Wear 77", Some(npc)), Some(true));
    assert_eq!(names(&s.me.take()), ["ACTION_FAIL"]);
}

// ---------------------------------------------------------------------------------- ReceivePremium

#[test]
fn receive_premium_without_items() {
    let mut s = Scene::new(40, 0, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("ReceivePremium", Some(npc)), Some(false));
    let ids: Vec<_> = messages(&s.me.take()).iter().map(|m| m.id).collect();
    assert_eq!(ids, [SystemMessageId::ThereAreNoMoreDimensionalItemsToBeFound]);
}

// ---------------------------------------------------------------------------------- voice / expoff

fn sent_texts(out: &[Vec<u8>]) -> Vec<String> {
    messages(out)
        .iter()
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

/// `.expoff` / `.expon`: the variable `EXPOFF` and the message; the same command twice says
/// nothing the second time, and the text is not said in chat.
#[test]
fn expoff_and_expon_switch_the_variable() {
    let mut s = Scene::new(20, 0, vec![]);
    s.say(".expoff");
    let out = s.me.take();
    assert_eq!(sent_texts(&out), ["Experience gain is disabled."]);
    assert!(!names(&out).contains(&"SAY2".to_owned()), "{:?}", names(&out));
    let player = s.world.player(ME).unwrap();
    assert_eq!(
        player.data.profile.variables.get("EXPOFF").map(String::as_str),
        Some("true")
    );
    assert!(player.data.profile.variables_changed.contains("EXPOFF"));

    s.say(".expoff");
    assert!(sent_texts(&s.me.take()).is_empty());

    s.say(".expon");
    assert_eq!(sent_texts(&s.me.take()), ["Experience gain is enabled."]);
    assert_eq!(
        s.world
            .player(ME)
            .unwrap()
            .data
            .profile
            .variables
            .get("EXPOFF")
            .map(String::as_str),
        Some("false")
    );
    s.say(".expon");
    assert!(sent_texts(&s.me.take()).is_empty());
}

/// With `EXPOFF` on, a living player gains no exp; a dead one still does (`onExperienceReceived`).
#[test]
fn expoff_stops_the_exp() {
    let mut s = Scene::new(20, 0, vec![]);
    let before = s.world.player(ME).unwrap().data.exp;
    s.say(".expoff");
    s.world.add_exp(ME, 1000);
    assert_eq!(s.world.player(ME).unwrap().data.exp, before);
    s.say(".expon");
    s.world.add_exp(ME, 1000);
    assert_ne!(s.world.player(ME).unwrap().data.exp, before);
}

/// The variable is restored at login with the message.
#[test]
fn expoff_is_restored_at_login() {
    let mut data = player_data(ME, 20, 0, vec![]);
    data.profile
        .variables
        .insert("EXPOFF".to_owned(), "true".to_owned());
    let root_log = Log::default();
    let mut world = World::new(WorldRng::from_seed(5), Box::new(NullOutbox));
    world.set_static_data(Box::new(self::data()));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(data),
        sink: Box::new(Sink(root_log.clone())),
        ip: None,
    });
    let out = root_log.take();
    assert!(sent_texts(&out).contains(&"Experience gain is disabled.".to_owned()));
}

/// `VoiceCommand`: `voice .expoff` runs the voiced handler and answers what it answers; anything
/// that is not `voice .word` answers `false`.
#[test]
fn voice_runs_a_voiced_command_from_a_page() {
    let mut s = Scene::new(20, 0, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("voice .expoff", Some(npc)), Some(true));
    assert_eq!(sent_texts(&s.me.take()), ["Experience gain is disabled."]);
    assert_eq!(s.bypass("voice .expon now", Some(npc)), Some(true));
    assert_eq!(sent_texts(&s.me.take()), ["Experience gain is enabled."]);
    assert_eq!(s.bypass("voice .nothing", Some(npc)), Some(false));
    assert_eq!(s.bypass("voice expoff", Some(npc)), Some(false));
    assert_eq!(s.bypass("voice", Some(npc)), Some(false));
}

// ---------------------------------------------------------------------------------- RentPet

fn rent_scene(adena: i64) -> (Scene, ObjectId) {
    let mut s = Scene::new(40, 0, vec![item(0x100, 57, adena)]);
    s.world.set_script_handler_settings(ScriptHandlerSettings {
        allow_rentpet: true,
        ..ScriptHandlerSettings::default()
    });
    let npc = s.npc(30827, 50);
    (s, npc)
}

#[test]
fn rent_pet_is_off_by_default() {
    let mut s = Scene::new(40, 0, vec![item(0x100, 57, 5000)]);
    let npc = s.npc(30827, 50);
    assert_eq!(s.bypass("RentPet", Some(npc)), Some(false));
}

#[test]
fn rent_pet_shows_its_prices() {
    let (mut s, npc) = rent_scene(5000);
    assert_eq!(s.bypass("RentPet", Some(npc)), Some(true));
    let pages = html(&s.me.take());
    assert_eq!(pages.len(), 1);
    assert!(pages[0].html.starts_with("<html><body>Pet Manager:<br>"));
    assert!(pages[0].html.contains(&format!("npc_{}_RentPet 11", npc.0)));
    assert!(!pages[0].html.contains("%objectId%"));
}

/// A wyvern for 30 seconds: 1800 adena, the gauge and the rent timer.
#[test]
fn rent_pet_mounts_and_charges() {
    let (mut s, npc) = rent_scene(5000);
    assert_eq!(s.bypass("RentPet 1", Some(npc)), Some(true));
    assert_eq!(s.count_of(57), 5000 - 1800);
    assert!(s.world.is_mounted(ME));
    assert!(s.world.is_rented_pet(ME));
    assert_eq!(s.world.mount_npc_id_of(ME), 12621);
    let out = s.me.take();
    let gauges: Vec<SetupGauge> = of(&out, ServerOpcode::SetupGauge);
    assert_eq!((gauges[0].color, gauges[0].time), (3, 30_000));
    // Rented for 30 seconds: afterwards the player walks again.
    s.advance(31_000);
    assert!(!s.world.is_mounted(ME));
}

#[test]
fn rent_pet_needs_the_adena() {
    let (mut s, npc) = rent_scene(100);
    assert_eq!(s.bypass("RentPet 1", Some(npc)), Some(true));
    assert!(!s.world.is_mounted(ME));
    assert_eq!(s.count_of(57), 100);
    assert_eq!(
        messages(&s.me.take()).iter().map(|m| m.id).collect::<Vec<_>>(),
        [SystemMessageId::YouDoNotHaveEnoughAdena]
    );
    // A bad number is an exception in Java: the handler answers false.
    assert_eq!(s.bypass("RentPet abc", Some(npc)), Some(false));
}

// ---------------------------------------------------------------------------------- FindPvP

#[test]
fn find_pvp_is_off_by_default_and_finds_nobody() {
    let mut s = Scene::new(40, 0, vec![]);
    let npc = s.npc(30003, 50);
    assert_eq!(s.bypass("FindPvP", Some(npc)), Some(false));
    assert!(s.me.take().is_empty());

    s.world.set_script_handler_settings(ScriptHandlerSettings {
        enable_find_pvp: true,
        ..ScriptHandlerSettings::default()
    });
    assert_eq!(s.bypass("FindPvP", Some(npc)), Some(false));
    let out = s.me.take();
    let says: Vec<&Vec<u8>> = out.iter().filter(|b| name_of(b) == "SAY2").collect();
    assert_eq!(says.len(), 1);
    // The whisper type (2) and the UTF-16 text.
    assert_eq!(i32::from_le_bytes(says[0][5..9].try_into().unwrap()), 2);
    let text: Vec<u8> = "Sorry, I can't find anyone in flag status right now."
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert!(says[0].windows(text.len()).any(|w| w == text.as_slice()));
}

// ---------------------------------------------------------------------------------- /mybirthday /unstuck

/// `MyBirthday`: month/day/year of `createDate` in `C1_S_BIRTHDAY_IS_S3_S4_S2` (the client orders
/// the parameters $s3/$s4/$s2).
#[test]
fn my_birthday_shows_the_create_date() {
    let mut data = player_data(ME, 20, 0, vec![]);
    data.profile.create_date = Some((2023, 7, 4));
    let mut s = Scene::with(data);
    s.user_command(126);
    let out = s.me.take();
    let sms = messages(&out);
    assert_eq!(sms.len(), 1);
    assert_eq!(sms[0].id, SystemMessageId::C1SBirthdayIsS3S4S2);
    assert_eq!(
        sms[0].params,
        [
            SystemMessageParam::PlayerName("P1".to_owned()),
            SystemMessageParam::Text("2023".to_owned()),
            SystemMessageParam::Text("7".to_owned()),
            SystemMessageParam::Text("4".to_owned()),
        ]
    );
}

/// With a plain interval (not 300) and no escape skill the animation is faked: the "You use
/// Escape" line, the Soul of Escape `MagicSkillUse` and the gauge; the teleport follows after the
/// time.
#[test]
fn unstuck_fakes_the_animation_and_teleports() {
    let mut s = Scene::new(20, 0, vec![]);
    s.world.set_script_handler_settings(ScriptHandlerSettings {
        unstuck_interval: 30,
        ..ScriptHandlerSettings::default()
    });
    s.user_command(52);
    let out = s.me.take();
    assert_eq!(sent_texts(&out), ["You use Escape: 30 seconds."]);
    let n = names(&out);
    assert!(n.contains(&"MAGIC_SKILL_USE".to_owned()), "{n:?}");
    let gauges: Vec<SetupGauge> = of(&out, ServerOpcode::SetupGauge);
    assert_eq!((gauges[0].color, gauges[0].time), (0, 30_000));
    assert!(s.world.is_casting_now(ME));

    // A second /unstuck while it runs does nothing.
    s.user_command(52);
    assert!(s.me.take().is_empty());

    s.advance(29_000);
    assert!(s.world.is_casting_now(ME));
    s.advance(2_000);
    assert!(!s.world.is_casting_now(ME));
}

#[test]
fn unstuck_is_refused_when_dead() {
    let mut s = Scene::new(20, 0, vec![]);
    s.world.set_current_hp(ME, 0.0, false);
    s.me.take();
    s.user_command(52);
    assert!(sent_texts(&s.me.take()).is_empty());
    assert!(!s.world.is_casting_now(ME));
}

// ---------------------------------------------------------------------------------- items

/// `RollingDice`: a number 1 to 6 for the player (`C1_HAS_ROLLED_A_S2`) and the `Dice` packet; the
/// flood protector refuses a second throw at once.
#[test]
fn rolling_dice_throws_and_is_flood_protected() {
    let mut s = Scene::new(20, 0, vec![item(0x100, 4625, 1)]);
    s.use_item(0x100);
    let out = s.me.take();
    let dice: Vec<Dice> = of(&out, ServerOpcode::Dice);
    assert_eq!(dice.len(), 1);
    assert_eq!(dice[0].object_id, ME.0);
    assert_eq!(dice[0].item_id, 4625);
    assert!((1..=6).contains(&dice[0].number));
    let sms = messages(&out);
    assert_eq!(sms[0].id, SystemMessageId::C1HasRolledAS2);
    assert_eq!(
        sms[0].params,
        [
            SystemMessageParam::Text("P1".to_owned()),
            SystemMessageParam::IntNumber(dice[0].number)
        ]
    );
    // The dice is not used up.
    assert_eq!(s.count_of(4625), 1);

    // The use-item protector (0.4 s) is over, the dice protector (4.2 s) is not.
    s.advance(1000);
    s.me.take();
    s.use_item(0x100);
    let out = s.me.take();
    assert!(of::<Dice>(&out, ServerOpcode::Dice).is_empty());
    assert!(
        messages(&out)
            .iter()
            .any(|m| m.id == SystemMessageId::YouMayNotThrowTheDiceAtThisTimeTryAgainLater)
    );
}

/// `CharmOfCourage`: the grade must reach the level's (D for 20 to 39; C needed from 40); the
/// charm is used up, the flag goes up and `EtcStatusUpdate` says so; death takes it again.
#[test]
fn charm_of_courage_is_used_and_lost_at_death() {
    let mut s = Scene::new(30, 0, vec![item(0x100, 9201, 1)]);
    s.use_item(0x100);
    let out = s.me.take();
    assert_eq!(s.count_of(9201), 0);
    assert!(s.world.has_charm_of_courage(ME));
    let etc: Vec<EtcStatusUpdate> = of(&out, ServerOpcode::EtcStatusUpdate);
    assert!(etc.iter().any(|e| e.charm_of_courage));

    // Death: the charm is used up and the client is told.
    s.world.do_die(ME, None);
    let out = s.me.take();
    assert!(!s.world.has_charm_of_courage(ME));
    let etc: Vec<EtcStatusUpdate> = of(&out, ServerOpcode::EtcStatusUpdate);
    assert!(etc.iter().any(|e| !e.charm_of_courage));
}

#[test]
fn charm_of_courage_of_a_low_grade_is_refused() {
    let mut s = Scene::new(45, 0, vec![item(0x100, 9201, 1)]);
    s.use_item(0x100);
    let out = s.me.take();
    assert_eq!(s.count_of(9201), 1);
    assert!(!s.world.has_charm_of_courage(ME));
    let sms = messages(&out);
    assert_eq!(sms[0].id, SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms);
    assert_eq!(sms[0].params, [SystemMessageParam::ItemName(9201)]);
}

/// `EventItem` outside a Block Checker match: "$s1 cannot be used due to unsuitable terms."
#[test]
fn event_item_outside_an_arena() {
    let mut s = Scene::new(45, 0, vec![item(0x100, 13787, 1)]);
    s.use_item(0x100);
    let out = s.me.take();
    assert_eq!(s.count_of(13787), 1);
    let sms = messages(&out);
    assert_eq!(sms[0].id, SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms);
    assert_eq!(sms[0].params, [SystemMessageParam::ItemName(13787)]);
}
