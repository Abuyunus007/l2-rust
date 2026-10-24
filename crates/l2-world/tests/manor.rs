#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The castle manor (M7-MANOR-01): loading the lists, the daily change of the period, the lord's
//! settings, the seed purchase and the crop sale, the `ManorManager` NPC, sowing and harvesting,
//! and the `SiegeDatabaseWrites` switch (D-51).
//! Java: managers/CastleManorManager.java, network/clientpackets/{RequestSetSeed,RequestSetCrop,
//! RequestBuySeed,RequestProcureCropList,RequestManorList}.java, network/serverpackets/{BuyListSeed,
//! ExShow*}.java, dist/game/data/scripts/ai/others/ManorManager/ManorManager.java,
//! handlers/items/{Seed,Harvester}.java, handlers/skill/effects/{Sow,Harvesting}.java,
//! entity/actor/Attackable.java. The expected values come from the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, Seeds, SkillLoadOptions, SkillTemplates,
    SkillTrees, SpawnData, SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::client::{
    CropSale, CropSaleItem, RequestBuySeed, RequestProcureCropList, RequestSetCrop, RequestSetSeed,
    SeedPurchase, SeedSale,
};
use l2_protocol_game::server::{
    BuyListSeed, CropOffer, CropProcureInfo, ExSendManorList, ExShowCropInfo, ExShowCropSetting,
    ExShowManorDefaultInfo, ExShowProcureCropDetail, ExShowSeedInfo, ExShowSeedSetting, ExShowSellCropList,
    ManorDefaultCrop, NpcHtmlMessage, ProcureCastle, SeedForSale, SeedProductionInfo, SellCropItem,
    SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord, ClanPrivilegesRef, PlayerClanState};
use l2_world::residence::{CastleRecord, ResidenceLoad};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Debug, Clone)]
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

struct Recorder(Log);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

const CLASS: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level>
<level val="10"><hp>300</hp><mp>150</mp><cp>60</cp></level><level val="30"><hp>600</hp><mp>300</mp><cp>120</cp></level>
<level val="40"><hp>800</hp><mp>400</mp><cp>160</cp></level></lvlUpgainData></list>"#;

const ADENA: i32 = 57;
/// Gludio's seed (Dark Coda) and the crop it grows, the crop's mature item, and the two rewards.
const SEED_1: i32 = 5016;
const CROP_1: i32 = 5073;
const MATURE_1: i32 = 5103;
const REWARD_1A: i32 = 1864;
const REWARD_1B: i32 = 1878;
/// Dion's seed, crop and mature item.
const SEED_2: i32 = 5017;
const CROP_2: i32 = 5068;
const HARVESTER: i32 = 5125;

/// Prices as `ItemData` has them: the seed 5 adena (so 3 to 50 for the lord), the crop 9 (5 to 90),
/// the rewards 10 and 20.
const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="5016" type="EtcItem" name="Seed: Dark Coda"><set name="weight" val="1" /><set name="price" val="5" />
  <set name="is_stackable" val="true" /><set name="handler" val="Seed" /><skills><skill id="2097" level="1" /></skills></item>
<item id="5017" type="EtcItem" name="Seed: Red Coda"><set name="weight" val="1" /><set name="price" val="5" />
  <set name="is_stackable" val="true" /><set name="handler" val="Seed" /><skills><skill id="2097" level="1" /></skills></item>
<item id="5073" type="EtcItem" name="Dark Coda"><set name="weight" val="2" /><set name="price" val="9" /><set name="is_stackable" val="true" /></item>
<item id="5068" type="EtcItem" name="Red Coda"><set name="weight" val="2" /><set name="price" val="9" /><set name="is_stackable" val="true" /></item>
<item id="5103" type="EtcItem" name="Mature Dark Coda"><set name="weight" val="2" /><set name="price" val="9" /><set name="is_stackable" val="true" /></item>
<item id="5098" type="EtcItem" name="Mature Red Coda"><set name="weight" val="2" /><set name="price" val="9" /><set name="is_stackable" val="true" /></item>
<item id="1864" type="EtcItem" name="Stem"><set name="weight" val="1" /><set name="price" val="10" /><set name="is_stackable" val="true" /></item>
<item id="1878" type="EtcItem" name="Braided Hemp"><set name="weight" val="1" /><set name="price" val="20" /><set name="is_stackable" val="true" /></item>
<item id="1865" type="EtcItem" name="Varnish"><set name="weight" val="1" /><set name="price" val="10" /><set name="is_stackable" val="true" /></item>
<item id="1879" type="EtcItem" name="Cokes"><set name="weight" val="1" /><set name="price" val="20" /><set name="is_stackable" val="true" /></item>
<item id="5125" type="EtcItem" name="Harvester"><set name="weight" val="20" /><set name="price" val="500" />
  <set name="handler" val="Harvester" /><skills><skill id="2098" level="1" /></skills></item>
</list>"#;

const SEEDS: &str = r#"<list>
<castle id="1">
  <crop id="5073" seedId="5016" mature_Id="5103" reward1="1864" reward2="1878" alternative="false" level="10" limit_seed="8100" limit_crops="9000" />
</castle>
<castle id="2">
  <crop id="5068" seedId="5017" mature_Id="5098" reward1="1865" reward2="1879" alternative="false" level="13" limit_seed="8100" limit_crops="9000" />
</castle>
</list>"#;

/// Sowing and Harvesting as the datapack has them, with one level and the same range and mana.
const SKILLS: &str = r##"<list>
<skill id="2097" levels="1" name="Sowing"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>40</castRange><effectRange>400</effectRange><hitTime>500</hitTime><isMagic>2</isMagic>
<magicLevel>10</magicLevel><mpConsume>4</mpConsume><nextActionAttack>true</nextActionAttack><reuseDelay>0</reuseDelay>
<effects><effect name="Sow" /></effects></skill>
<skill id="2098" levels="1" name="Harvesting"><operateType>A1</operateType><targetType>CORPSE_MOB</targetType>
<castRange>20</castRange><effectRange>400</effectRange><hitTime>500</hitTime><isMagic>2</isMagic>
<magicLevel>1</magicLevel><mpConsume>3</mpConsume><reuseDelay>0</reuseDelay>
<effects><effect name="Harvesting" /></effects></skill>
</list>"##;

const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>"#;

/// 9100 a monster that can be sown with a drop list, 9101 a monster that cannot, 9102 a raid boss,
/// 9103 a chest, 9104 a strong monster (skill 4305: x4); 35644 and 35645 the manor managers of
/// castle 1 and 2, 35100 the chamberlain of castle 1.
fn npcs() -> String {
    format!(
        r#"<list>
  <npc id="9100" type="Monster" level="10" name="Sowable">{STATS}<status canBeSown="true" />
    <dropLists><drop><item id="57" min="5" max="5" chance="100"/><item id="1864" min="1" max="1" chance="100"/></drop></dropLists>
  </npc>
  <npc id="9101" type="Monster" level="10" name="Not Sowable">{STATS}</npc>
  <npc id="9102" type="RaidBoss" level="10" name="Boss">{STATS}<status canBeSown="true" /></npc>
  <npc id="9103" type="Chest" level="10" name="Chest">{STATS}<status canBeSown="true" /></npc>
  <npc id="9104" type="Monster" level="30" name="Strong">{STATS}<status canBeSown="true" />
    <skillList><skill id="4305" level="1" /></skillList></npc>
  <npc id="35644" type="Merchant" level="70" name="Manor Manager" title="Gludio">
    <parameters><param name="manor_id" value="1" /></parameters>{STATS}</npc>
  <npc id="35645" type="Merchant" level="70" name="Manor Manager" title="Dion">
    <parameters><param name="manor_id" value="2" /></parameters>{STATS}</npc>
  <npc id="35100" type="Folk" level="70" name="Sayres" title="Chamberlain">{STATS}</npc>
</list>"#
    )
}

/// The ManorManager pages and the default page, written into the datapack folder of the test.
const PAGE_MANAGER: &str = "<html><body>Manor %objectId% %playername% <a action=\"bypass Script ManorManager manager-help-01.htm\">Help</a></body></html>";
const PAGE_LORD: &str = "<html><body>My Lord %objectId%</body></html>";
const PAGE_HELP: &str = "<html><body>Help 1 %objectId%</body></html>";
const PAGE_DEFAULT: &str = "<html><body>Default page</body></html>";

const X: i32 = -150_000;
const Y: i32 = 170_000;
const Z: i32 = -3_000;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
/// The leader of the Lions (owner of Gludio), a member without rights, and a farmer without a clan.
const LORD: i32 = 0x6000_0001;
const MEMBER: i32 = 0x6000_0002;
const FARMER: i32 = 0x6000_0003;
/// The leader of the Wolves, who own Dion.
const DION_LORD: i32 = 0x6000_0004;

/// Map region of tile (15, 23) is Gludio's area (castle 1) and the next tile Dion's (castle 2).
const REGIONS: &str = r#"<list>
  <region name="gludio_area" town="Gludio" castle="1" locId="910" bbs="1"><respawnPoint X="1" Y="2" Z="3" /><map X="15" Y="23" /></region>
  <region name="dion_area" town="Dion" castle="2" locId="911" bbs="2"><respawnPoint X="1" Y="2" Z="3" /><map X="16" Y="23" /></region>
</list>"#;

/// The town of Gludio pays its tax to castle 1 (`taxById`).
const ZONES: &str = r#"<?xml version="1.0"?>
<list enabled="true">
  <zone name="gludio_town" type="TownZone" shape="Cuboid" minZ="-5000" maxZ="5000">
    <node X="-153000" Y="168000" /><node X="-147000" Y="172000" /><stat name="taxById" val="1" /></zone>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
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
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.0.skills.iter().any(|s| s.id == skill_id && s.is_passive())
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
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

// ---------------------------------------------------------------------------------- scene

struct Options {
    siege_database_writes: bool,
    manor: ManorSettings,
    settings: WorldSettings,
    /// The two castles' treasuries and the Lions' clan warehouse (an adena stack).
    treasury_1: i64,
    treasury_2: i64,
    lions_warehouse: Vec<ItemData>,
    /// The seed of the world's RNG.
    seed: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            siege_database_writes: false,
            manor: ManorSettings::default(),
            settings: WorldSettings::default(),
            treasury_1: 1_000_000,
            treasury_2: 1_000_000,
            lions_warehouse: Vec::new(),
            seed: 7,
        }
    }
}

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

fn member(id: i32, power_grade: i32) -> ClanMemberRecord {
    ClanMemberRecord {
        object_id: id,
        name: format!("P{}", id - 0x6000_0000),
        level: 40,
        power_grade,
        ..ClanMemberRecord::default()
    }
}

fn clan(
    clan_id: i32,
    name: &str,
    castle_id: i32,
    leader: i32,
    other: i32,
    warehouse: Vec<ItemData>,
) -> ClanRecord {
    ClanRecord {
        warehouse,
        clan_id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level: 5,
            castle_id,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members: vec![member(leader, 1), member(other, 5)],
        ..ClanRecord::default()
    }
}

fn castle(id: i32, name: &str, owner_id: i32, treasury: i64) -> CastleRecord {
    CastleRecord {
        id,
        found: true,
        name: name.to_owned(),
        tax_percent: 10,
        treasury,
        time_registration_over: true,
        owner_id,
        ..CastleRecord::default()
    }
}

impl Scene {
    fn new(options: Options) -> Self {
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
        write("data/mapregion/regions.xml", REGIONS);
        write("data/zones/town.xml", ZONES);
        let script = "data/scripts/ai/others/ManorManager";
        write(&format!("{script}/manager.htm"), PAGE_MANAGER);
        write(&format!("{script}/manager-lord.htm"), PAGE_LORD);
        write(&format!("{script}/manager-help-01.htm"), PAGE_HELP);
        write("data/html/npcdefault.htm", PAGE_DEFAULT);

        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(options.seed), Box::new(Recorder(log.clone())));
        world.set_settings(options.settings);
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_exp_data(
            Arc::new(ExperienceTable::from_xml(r#"<table maxLevel="85" maxPetLevel="85"></table>"#).unwrap()),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
        );
        world.set_regen_settings(RegenSettings {
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(&npcs(), &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        world.set_residence_settings(ResidenceSettings {
            siege_database_writes: options.siege_database_writes,
            ..ResidenceSettings::default()
        });
        world.load_clans(
            vec![
                clan(LIONS, "Lions", 1, LORD, MEMBER, options.lions_warehouse),
                clan(WOLVES, "Wolves", 2, DION_LORD, 0x6000_0005, Vec::new()),
            ],
            Vec::new(),
        );
        world.load_residences(ResidenceLoad {
            castles: vec![
                castle(1, "Gludio", LIONS, options.treasury_1),
                castle(2, "Dion", WOLVES, options.treasury_2),
            ],
            ..ResidenceLoad::default()
        });
        world.set_manor_settings(options.manor);
        let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
        world.set_manor_seeds(Arc::new(Seeds::from_xml(SEEDS, &items).unwrap()));
        Self {
            world,
            log,
            _root: root,
        }
    }

    /// The default scene: the manor loaded at noon on a Monday (APPROVED), no rows.
    fn standard() -> Self {
        let mut s = Self::new(Options::default());
        s.load_at(ManorLoad::default(), at(12, 0));
        s
    }

    fn load_at(&mut self, load: ManorLoad, now_ms: i64) {
        self.world.load_manor_at(load, now_ms, utc);
        self.log.take();
    }

    fn enter(&mut self, id: i32, level: i32, items: Vec<ItemData>) {
        self.enter_at(id, level, items, (X, Y, Z));
    }

    fn enter_at(&mut self, id: i32, level: i32, items: Vec<ItemData>, (x, y, z): (i32, i32, i32)) {
        let clan_id = match id {
            LORD | MEMBER => LIONS,
            DION_LORD => WOLVES,
            _ => 0,
        };
        let mut data = PlayerData {
            object_id: ObjectId(id),
            account: format!("acc{id}"),
            name: format!("P{}", id - 0x6000_0000),
            title: String::new(),
            class_id: 0,
            base_class: 0,
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
            x,
            y,
            z,
            heading: 0,
            cur_hp: 100.0,
            max_hp: 100.0,
            cur_mp: 50.0,
            max_mp: 50.0,
            cur_cp: 20.0,
            max_cp: 20.0,
            online_time: 0,
            items,
            skills: Vec::new(),
            shortcuts: Vec::new(),
            profile: PlayerProfile::default(),
        };
        if clan_id != 0 {
            data.profile.clan = Some(PlayerClanState {
                clan_id,
                // The leaders hold every right, the member none.
                privileges: ClanPrivilegesRef::Own(if id == MEMBER { 0 } else { 0x00FF_FFFF }),
                ..PlayerClanState::default()
            });
        }
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.ticks(20);
        self.log.take();
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn take(&mut self) -> Vec<Out> {
        self.log.take()
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    fn manor(&mut self, player: i32, input: ManorInput) {
        self.input(player, ClientInput::Manor(input));
    }

    /// The player clicks the NPC twice (the first selects it, the second talks).
    fn talk_to(&mut self, player: i32, npc: ObjectId) {
        for _ in 0..2 {
            self.input(
                player,
                ClientInput::Action {
                    object_id: npc.0,
                    action_id: 0,
                },
            );
            self.ticks(2);
        }
    }

    fn npc(&mut self, npc_id: i32, dx: i32) -> ObjectId {
        self.world.spawn_npc_at(npc_id, X + dx, Y, Z, 0).unwrap()
    }

    fn adena(&self, who: i32) -> i64 {
        self.count(who, ADENA)
    }

    fn count(&self, who: i32, item_id: i32) -> i64 {
        self.world
            .player(ObjectId(who))
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id && i.loc == "INVENTORY")
            .map(|i| i.count)
            .sum()
    }

    fn treasury(&self, castle_id: i32) -> i64 {
        self.world.castle(CastleId(castle_id)).unwrap().treasury
    }

    fn warehouse_count(&self, clan_id: i32, item_id: i32) -> i64 {
        self.world
            .clan(clan_id)
            .unwrap()
            .warehouse
            .iter()
            .filter(|i| i.item_id == item_id)
            .map(|i| i.count)
            .sum()
    }
}

fn item(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".into(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

fn adena(count: i64) -> ItemData {
    item(0x6000_2000, ADENA, count)
}

/// 2026-01-05, a Monday, `hour:min:00` in UTC (the offset of the tests).
fn at(hour: i64, min: i64) -> i64 {
    1_767_571_200_000 + hour * 3_600_000 + min * 60_000
}

fn utc(_ms: i64) -> i64 {
    0
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn packets_to(out: &[Out], to: i32, name: &str) -> Vec<Vec<u8>> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(t, b) if *t == to && name_of(b) == name => Some(b.clone()),
            _ => None,
        })
        .collect()
}

fn decoded<P: Decode>(out: &[Out], to: i32, name: &str) -> Vec<P> {
    packets_to(out, to, name).iter().map(|b| decode::<P>(b)).collect()
}

fn messages_to(out: &[Out], to: i32) -> Vec<SystemMessage> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
}

fn message_ids(out: &[Out], to: i32) -> Vec<SystemMessageId> {
    messages_to(out, to).into_iter().map(|m| m.id).collect()
}

fn action_failed(out: &[Out], to: i32) -> usize {
    packets_to(out, to, "ACTION_FAIL").len()
}

fn manor_writes(out: &[Out]) -> Vec<ManorWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Manor(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn residence_writes(out: &[Out]) -> Vec<residence::ResidenceWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Residence(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn item_ops(out: &[Out]) -> Vec<ItemRowOp> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemRows { ops, .. }) => Some(ops.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn production(
    castle_id: i32,
    seed_id: i32,
    amount: i64,
    start: i64,
    price: i64,
    next: bool,
) -> ManorProductionRow {
    ManorProductionRow {
        castle_id,
        seed_id,
        amount,
        start_amount: start,
        price,
        next_period: next,
    }
}

fn procure(
    castle_id: i32,
    crop_id: i32,
    amount: i64,
    start: i64,
    price: i64,
    reward: i32,
    next: bool,
) -> ManorProcureRow {
    ManorProcureRow {
        castle_id,
        crop_id,
        amount,
        start_amount: start,
        price,
        reward_type: reward,
        next_period: next,
    }
}

fn html_to(out: &[Out], to: i32) -> Vec<NpcHtmlMessage> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
}

// ---------------------------------------------------------------------------------- loading

/// `CastleManorManager` constructor: the rows of every castle, both periods, the unknown seed and
/// crop ids dropped with a warning, a castle without rows has empty lists.
#[test]
fn the_rows_are_sorted_into_the_castles_and_periods() {
    let mut s = Scene::new(Options::default());
    s.load_at(
        ManorLoad {
            production: vec![
                production(1, SEED_1, 40, 100, 7, false),
                production(1, SEED_1, 100, 100, 8, true),
                // 9999 is no seed of Seeds.xml.
                production(1, 9999, 1, 1, 1, false),
                production(2, SEED_2, 5, 10, 6, false),
            ],
            procure: vec![
                procure(1, CROP_1, 20, 50, 9, 2, false),
                procure(1, CROP_1, 50, 50, 10, 1, true),
                procure(1, 8888, 1, 1, 1, 1, false),
            ],
        },
        at(12, 0),
    );
    let w = &s.world;
    let current = w.manor_seed_production(1, false).unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(
        (
            current[0].id,
            current[0].amount,
            current[0].start_amount,
            current[0].price
        ),
        (SEED_1, 40, 100, 7)
    );
    assert_eq!(w.manor_seed_product(1, SEED_1, true).unwrap().price, 8);
    assert_eq!(w.manor_seed_production(2, false).unwrap().len(), 1);
    assert_eq!(w.manor_seed_production(2, true).unwrap().len(), 0);
    assert_eq!(w.manor_crop_procure_of(1, CROP_1, false).unwrap().reward, 2);
    assert_eq!(w.manor_crop_procure_of(1, CROP_1, true).unwrap().price, 10);
    assert_eq!(
        w.manor_crop_procure(1, false).unwrap().len(),
        1,
        "the unknown crop is dropped"
    );
    // A castle the manor never saw has no lists (Java's `null`).
    assert!(w.manor_seed_production(3, false).is_none());
}

/// `getManorCost`: the seed reference price times the start amount of every seed, plus the price
/// times the start amount of every crop.
#[test]
fn the_cost_of_a_period_is_javas_sum() {
    let mut s = Scene::new(Options::default());
    s.load_at(
        ManorLoad {
            production: vec![
                production(1, SEED_1, 40, 100, 7, false),
                production(1, SEED_1, 40, 30, 7, true),
            ],
            procure: vec![procure(1, CROP_1, 20, 50, 9, 2, false)],
        },
        at(12, 0),
    );
    // The seed's reference price is 5: 5 * 100 + 9 * 50.
    assert_eq!(s.world.manor_cost(1, false), 5 * 100 + 9 * 50);
    // The next period: 5 * 30 and no crops.
    assert_eq!(s.world.manor_cost(1, true), 150);
    assert_eq!(s.world.manor_cost(2, false), 0);
}

/// `AllowManor = False`: the mode is DISABLED, nothing is loaded and no change is scheduled.
#[test]
fn a_disabled_manor_loads_nothing() {
    let mut s = Scene::new(Options {
        manor: ManorSettings {
            allow_manor: false,
            ..ManorSettings::default()
        },
        ..Options::default()
    });
    s.load_at(
        ManorLoad {
            production: vec![production(1, SEED_1, 40, 100, 7, false)],
            procure: Vec::new(),
        },
        at(12, 0),
    );
    assert_eq!(s.world.manor_current_mode_name(), "DISABLED");
    assert_eq!(s.world.manor_next_mode_change_text_at(utc), "Disabled");
    assert!(!s.world.manor_has_mode_timer());
    assert!(s.world.manor_seed_production(1, false).is_none());
}

/// The mode follows the time of the day and the next change is scheduled on the wheel.
#[test]
fn the_mode_is_set_from_the_time_and_the_change_is_scheduled() {
    let mut s = Scene::new(Options::default());
    s.load_at(ManorLoad::default(), at(12, 0));
    assert_eq!(s.world.manor_current_mode_name(), "APPROVED");
    assert!(s.world.manor_is_approved());
    // 20:00 the same day.
    assert_eq!(s.world.manor_next_mode_change_text_at(utc), "05/01 20:00:00");
    assert!(s.world.manor_has_mode_timer());

    let mut s = Scene::new(Options::default());
    s.load_at(ManorLoad::default(), at(22, 30));
    assert!(s.world.manor_is_modifiable());
    assert_eq!(s.world.manor_next_mode_change_text_at(utc), "06/01 04:30:00");

    let mut s = Scene::new(Options::default());
    s.load_at(ManorLoad::default(), at(20, 3));
    assert!(s.world.manor_is_under_maintenance());
    assert_eq!(s.world.manor_next_mode_change_text_at(utc), "05/01 20:06:00");
}

/// The change fires on the timer wheel when its time has come, and schedules the one after it.
#[test]
fn the_wheel_changes_the_mode() {
    let mut s = Scene::new(Options::default());
    // Maintenance since 20:00; the change is due at 20:06, five seconds from now.
    s.load_at(ManorLoad::default(), at(20, 5) + 55_000);
    assert!(s.world.manor_is_under_maintenance());
    // The real clock of the timer is not the test's: drive the change the way the timer does.
    s.world.manor_change_mode_at(at(20, 6), utc);
    assert!(s.world.manor_is_modifiable());
    assert_eq!(s.world.manor_next_mode_change_text_at(utc), "06/01 04:30:00");
}

// ---------------------------------------------------------------------------------- the change at 20:00

fn two_castle_rows() -> ManorLoad {
    ManorLoad {
        production: vec![
            // The period that ends: 60 of 100 seeds sold.
            production(1, SEED_1, 40, 100, 7, false),
            // The next period the lord set: 30 seeds at 7.
            production(1, SEED_1, 30, 30, 7, true),
        ],
        procure: vec![
            // The castle wanted 50 crops at 9 and bought 20 of them.
            procure(1, CROP_1, 30, 50, 9, 1, false),
            procure(1, CROP_1, 80, 80, 9, 1, true),
        ],
    }
}

/// `changeMode` APPROVED: the crops bought go to the clan warehouse (`(start - amount) * 0.9`),
/// what was not bought goes to the treasury (`amount * price`), the next period is the current one
/// with its amounts reset, and `storeMe` writes both tables. With `SiegeDatabaseWrites` all of it
/// goes out.
#[test]
fn the_change_at_eight_in_the_evening_pays_out_with_the_switch() {
    let mut s = Scene::new(Options {
        siege_database_writes: true,
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(19, 59) + 59_000);
    s.world.manor_change_mode_at(at(20, 0), utc);
    let out = s.take();
    assert!(s.world.manor_is_under_maintenance());

    // (50 - 30) * 0.9 = 18 mature crops for the Lions' warehouse.
    assert_eq!(s.warehouse_count(LIONS, MATURE_1), 18);
    let ops = item_ops(&out);
    assert!(
        ops.iter()
            .any(|op| matches!(op, ItemRowOp::Insert { owner_id, item }
            if *owner_id == LIONS && item.item_id == MATURE_1 && item.count == 18 && item.loc == "CLANWH")),
        "{ops:?}"
    );
    // The 30 crops nobody sold are paid for: 30 * 9 = 270.
    assert_eq!(s.treasury(1), 1_000_000 + 270);
    assert!(
        residence_writes(&out).contains(&residence::ResidenceWrite::CastleTreasury {
            castle_id: 1,
            treasury: 1_000_270
        })
    );

    // The next period is the current one now, with the amounts reset to the start amounts.
    let current = s.world.manor_seed_production(1, false).unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!((current[0].amount, current[0].start_amount), (30, 30));
    let crops = s.world.manor_crop_procure(1, false).unwrap();
    assert_eq!((crops[0].amount, crops[0].start_amount), (80, 80));
    // The treasury (1,000,270) pays for the period (5 * 30 + 9 * 80 = 870), so the next period
    // keeps its lists.
    assert_eq!(s.world.manor_seed_production(1, true).unwrap().len(), 1);

    // storeMe: the current period of every castle, then the next.
    let writes = manor_writes(&out);
    let [ManorWrite::StoreAll { production, procure }] = writes.as_slice() else {
        panic!("{writes:?}");
    };
    assert_eq!(production.iter().filter(|r| !r.next_period).count(), 1);
    assert_eq!(production.iter().filter(|r| r.next_period).count(), 1);
    assert_eq!(procure.len(), 2);
    assert!(production.iter().all(|r| r.castle_id == 1));
    // Dion's lists are empty.
    assert!(s.world.manor_seed_production(2, false).unwrap().is_empty());
}

/// Without `SiegeDatabaseWrites` the Java game server does the same at the same hour: this world
/// moves the lists on, but touches no clan warehouse row, no treasury row and no manor table.
#[test]
fn the_change_at_eight_in_the_evening_writes_nothing_without_the_switch() {
    let mut s = Scene::new(Options::default());
    s.load_at(two_castle_rows(), at(19, 59) + 59_000);
    s.world.manor_change_mode_at(at(20, 0), utc);
    let out = s.take();
    assert!(s.world.manor_is_under_maintenance());
    // The lists moved on.
    assert_eq!(s.world.manor_crop_procure(1, false).unwrap()[0].amount, 80);
    // The treasury is this world's copy, not written.
    assert_eq!(s.treasury(1), 1_000_000 + 270);
    assert!(residence_writes(&out).is_empty());
    // The warehouse and the tables are Sieghardt's.
    assert_eq!(s.warehouse_count(LIONS, MATURE_1), 0);
    assert!(item_ops(&out).is_empty());
    assert!(manor_writes(&out).is_empty());
}

/// `changeMode` APPROVED: when the treasury cannot pay for the period that starts, its lists are
/// emptied (`getManorCost(castleId, false) > treasury`).
#[test]
fn a_poor_castle_loses_the_next_period_at_eight_in_the_evening() {
    let mut s = Scene::new(Options {
        treasury_1: 100,
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(19, 59) + 59_000);
    s.world.manor_change_mode_at(at(20, 0), utc);
    // 100 + 270 = 370 < 5 * 30 + 9 * 80 = 870.
    assert!(s.world.manor_seed_production(1, true).unwrap().is_empty());
    assert!(s.world.manor_crop_procure(1, true).unwrap().is_empty());
    // The current period is what the lord had set (not reset: Java leaves the amounts).
    assert_eq!(s.world.manor_seed_production(1, false).unwrap().len(), 1);
}

/// `changeMode` MAINTENANCE: the leaders of the clans that own castles are told, online ones only.
#[test]
fn the_leader_hears_that_the_manor_was_updated() {
    let mut s = Scene::new(Options::default());
    s.load_at(ManorLoad::default(), at(20, 3));
    s.enter(LORD, 40, vec![]);
    s.enter(MEMBER, 40, vec![]);
    s.world.manor_change_mode_at(at(20, 6), utc);
    let out = s.take();
    assert!(s.world.manor_is_modifiable());
    assert_eq!(
        message_ids(&out, LORD),
        [SystemMessageId::TheManorInformationHasBeenUpdated]
    );
    assert!(message_ids(&out, MEMBER).is_empty(), "only the leader is told");
}

/// `changeMode` MODIFIABLE: the period the lord set is paid from the treasury; a poor castle with
/// a full clan warehouse loses it and the leader is told. (The `&&` of Java: with room in the
/// warehouse a short treasury is not enough to lose it, and the payment fails quietly.)
#[test]
fn the_next_period_is_paid_when_it_is_approved() {
    let mut s = Scene::new(Options {
        siege_database_writes: true,
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(23, 30));
    assert!(s.world.manor_is_modifiable());
    s.world.manor_change_mode_at(at(4, 30), utc);
    let out = s.take();
    assert!(s.world.manor_is_approved());
    // The next period costs 5 * 30 + 9 * 80 = 870.
    assert_eq!(s.treasury(1), 1_000_000 - 870);
    assert!(
        residence_writes(&out).contains(&residence::ResidenceWrite::CastleTreasury {
            castle_id: 1,
            treasury: 999_130
        })
    );
    assert_eq!(s.world.manor_seed_production(1, true).unwrap().len(), 1);
    // `AltManorSaveAllActions` is off: no table write now.
    assert!(manor_writes(&out).is_empty());
}

#[test]
fn a_poor_castle_with_a_full_warehouse_loses_the_next_period() {
    let mut s = Scene::new(Options {
        treasury_1: 10,
        // The clan warehouse holds more than the limit: `validateCapacity(0)` fails.
        settings: WorldSettings {
            warehouse_slots_clan: 0,
            ..WorldSettings::default()
        },
        // The Lions' warehouse holds one item, so it is over a limit of 0.
        lions_warehouse: vec![ItemData {
            loc: "CLANWH".into(),
            ..item(0x6000_1000, ADENA, 5)
        }],
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(23, 30));
    s.enter(LORD, 40, vec![]);
    s.world.manor_change_mode_at(at(4, 30), utc);
    let out = s.take();
    assert!(s.world.manor_seed_production(1, true).unwrap().is_empty());
    assert!(s.world.manor_crop_procure(1, true).unwrap().is_empty());
    assert_eq!(
        message_ids(&out, LORD),
        [SystemMessageId::YouDoNotHaveEnoughFundsInTheClanWarehouseForTheManorToOperate]
    );
    // Nothing was paid.
    assert_eq!(s.treasury(1), 10);
}

/// `storeMe` on the automatic save and at shutdown: a job of the manor, only with the switch.
#[test]
fn store_me_follows_the_switch() {
    let mut on = Scene::new(Options {
        siege_database_writes: true,
        ..Options::default()
    });
    on.load_at(two_castle_rows(), at(12, 0));
    assert!(on.world.manor_store_me());
    assert_eq!(manor_writes(&on.take()).len(), 1);

    let mut off = Scene::new(Options::default());
    off.load_at(two_castle_rows(), at(12, 0));
    assert!(!off.world.manor_store_me());
    assert!(manor_writes(&off.take()).is_empty());
}

/// `resetManorData`: a castle that loses its owner loses its lists (the rows follow only with
/// `AltManorSaveAllActions` and the switch).
#[test]
fn a_castle_that_loses_its_owner_loses_its_manor() {
    let mut s = Scene::new(Options {
        siege_database_writes: true,
        manor: ManorSettings {
            save_all_actions: true,
            ..ManorSettings::default()
        },
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(12, 0));
    s.world.castle_remove_owner(CastleId(1), Some(LIONS));
    let out = s.take();
    assert!(s.world.manor_seed_production(1, false).unwrap().is_empty());
    assert!(s.world.manor_seed_production(1, true).unwrap().is_empty());
    assert!(s.world.manor_crop_procure(1, false).unwrap().is_empty());
    assert_eq!(manor_writes(&out), [ManorWrite::DeleteCastle { castle_id: 1 }]);
}

#[test]
fn the_reset_rows_need_the_switch() {
    let mut s = Scene::new(Options {
        manor: ManorSettings {
            save_all_actions: true,
            ..ManorSettings::default()
        },
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(12, 0));
    s.world.castle_remove_owner(CastleId(1), Some(LIONS));
    assert!(s.world.manor_seed_production(1, false).unwrap().is_empty());
    assert!(manor_writes(&s.take()).is_empty());
}

// ---------------------------------------------------------------------------------- RequestManorList

#[test]
fn the_manor_list_names_the_castles_in_lower_case() {
    let mut s = Scene::standard();
    s.enter(FARMER, 10, vec![]);
    s.manor(FARMER, ManorInput::ManorList);
    let out = s.take();
    let lists = decoded::<ExSendManorList>(&out, FARMER, "EX_SEND_MANOR_LIST");
    assert_eq!(
        lists,
        [ExSendManorList {
            castles: vec![(1, "gludio".to_owned()), (2, "dion".to_owned())]
        }]
    );
}

// ---------------------------------------------------------------------------------- the lord

/// A scene at 23:30 (the lords may set the period) with the lord next to the Gludio manager, who
/// he has talked to.
fn lord_scene(options: Options) -> (Scene, ObjectId) {
    let mut s = Scene::new(options);
    s.load_at(two_castle_rows(), at(23, 30));
    assert!(s.world.manor_is_modifiable());
    s.enter(LORD, 40, vec![]);
    let manager = s.npc(35644, 20);
    s.talk_to(LORD, manager);
    s.take();
    (s, manager)
}

fn seed_sale(item_id: i32, sales: i64, price: i64) -> SeedSale {
    SeedSale {
        item_id,
        sales,
        price,
    }
}

/// `RequestSetSeed`: the list is filtered by the seed's limit and its price range
/// (`[0.6 * reference, 10 * reference]`, here 3 to 50), the entries are stored as the next period
/// with `amount = startAmount = sales`.
#[test]
fn the_lord_sets_the_seeds_of_the_next_period() {
    let (mut s, _) = lord_scene(Options::default());
    s.manor(
        LORD,
        ManorInput::SetSeed(RequestSetSeed {
            manor_id: 1,
            items: vec![
                seed_sale(SEED_1, 100, 7),
                // Under the minimum price, over the limit (8100), an unknown seed.
                seed_sale(SEED_1, 100, 2),
                seed_sale(SEED_1, 9_000, 7),
                seed_sale(9_999, 10, 7),
                // NOTE(java-quirk): Java does not check that the seed is the castle's own.
                seed_sale(SEED_2, 50, 10),
            ],
        }),
    );
    let out = s.take();
    let next = s.world.manor_seed_production(1, true).unwrap();
    assert_eq!(
        next,
        [
            SeedProduction {
                id: SEED_1,
                amount: 100,
                price: 7,
                start_amount: 100
            },
            SeedProduction {
                id: SEED_2,
                amount: 50,
                price: 10,
                start_amount: 50
            },
        ]
    );
    // The current period is untouched.
    assert_eq!(s.world.manor_seed_production(1, false).unwrap().len(), 1);
    // `AltManorSaveAllActions` is off: no rows now.
    assert!(manor_writes(&out).is_empty());
    assert_eq!(action_failed(&out, LORD), 0);
}

#[test]
fn the_lords_seeds_are_written_at_once_with_save_all_actions() {
    // A player's action: written without `SiegeDatabaseWrites`.
    let (mut s, _) = lord_scene(Options {
        manor: ManorSettings {
            save_all_actions: true,
            ..ManorSettings::default()
        },
        ..Options::default()
    });
    s.manor(
        LORD,
        ManorInput::SetSeed(RequestSetSeed {
            manor_id: 1,
            items: vec![seed_sale(SEED_1, 100, 7)],
        }),
    );
    assert_eq!(
        manor_writes(&s.take()),
        [ManorWrite::ReplaceNextProduction {
            castle_id: 1,
            rows: vec![production(1, SEED_1, 100, 100, 7, true)],
        }]
    );
}

fn crop_sale(item_id: i32, sales: i64, price: i64, reward_type: i8) -> CropSale {
    CropSale {
        item_id,
        sales,
        price,
        reward_type,
    }
}

/// `RequestSetCrop`: only the crops the castle's seeds grow (`getSeedByCrop(cropId, manorId)`),
/// within the crop's limit and price range (here 5 to 90).
#[test]
fn the_lord_sets_the_crops_of_the_next_period() {
    let (mut s, _) = lord_scene(Options {
        manor: ManorSettings {
            save_all_actions: true,
            ..ManorSettings::default()
        },
        ..Options::default()
    });
    s.manor(
        LORD,
        ManorInput::SetCrop(RequestSetCrop {
            manor_id: 1,
            items: vec![
                crop_sale(CROP_1, 100, 20, 2),
                // Dion's crop is not Gludio's, and a price under the minimum.
                crop_sale(CROP_2, 100, 20, 1),
                crop_sale(CROP_1, 100, 4, 1),
            ],
        }),
    );
    let out = s.take();
    assert_eq!(
        s.world.manor_crop_procure(1, true).unwrap(),
        [CropProcure {
            id: CROP_1,
            amount: 100,
            reward: 2,
            start_amount: 100,
            price: 20
        }]
    );
    assert_eq!(
        manor_writes(&out),
        [ManorWrite::ReplaceNextProcure {
            castle_id: 1,
            rows: vec![procure(1, CROP_1, 100, 100, 20, 2, true)],
        }]
    );
}

/// The checks of the two setting packets, in Java's order: the period, the clan and its right, the
/// NPC within reach.
#[test]
fn only_the_lord_may_set_the_manor_and_only_in_the_modifiable_period() {
    let seeds = |manor_id| {
        ManorInput::SetSeed(RequestSetSeed {
            manor_id,
            items: vec![seed_sale(SEED_1, 100, 7)],
        })
    };
    // Not the modifiable period: ActionFailed.
    let mut s = Scene::new(Options::default());
    s.load_at(two_castle_rows(), at(12, 0));
    s.enter(LORD, 40, vec![]);
    let manager = s.npc(35644, 20);
    s.talk_to(LORD, manager);
    s.take();
    s.manor(LORD, seeds(1));
    let out = s.take();
    assert_eq!(action_failed(&out, LORD), 1);
    assert_eq!(s.world.manor_seed_production(1, true).unwrap().len(), 1);
    assert_eq!(s.world.manor_seed_product(1, SEED_1, true).unwrap().price, 7);

    // A member without the CASTLE_MANOR right.
    let (mut s, manager) = lord_scene(Options::default());
    s.enter(MEMBER, 40, vec![]);
    s.talk_to(MEMBER, manager);
    s.take();
    s.manor(MEMBER, seeds(1));
    assert_eq!(action_failed(&s.take(), MEMBER), 1);
    // The leader of another castle's clan sets another castle's manor.
    s.enter(DION_LORD, 40, vec![]);
    s.talk_to(DION_LORD, manager);
    s.take();
    s.manor(DION_LORD, seeds(1));
    assert_eq!(action_failed(&s.take(), DION_LORD), 1);
    // Out of reach of the NPC.
    s.world.set_target(ObjectId(LORD), None);
    let far = s.world.spawn_npc_at(35644, X + 5000, Y, Z, 0).unwrap();
    s.input(
        LORD,
        ClientInput::Action {
            object_id: far.0,
            action_id: 0,
        },
    );
    s.ticks(2);
    s.take();
    s.manor(LORD, seeds(1));
    assert_eq!(action_failed(&s.take(), LORD), 1);
    // The list was never replaced by any of these (the lord's own row is the loaded one).
    assert_eq!(s.world.manor_seed_product(1, SEED_1, true).unwrap().price, 7);
}

/// Java throws when the lord never talked to an NPC (`getLastFolkNPC()` is null): nothing is
/// answered and nothing changes.
#[test]
fn a_setting_without_an_npc_changes_nothing() {
    let mut s = Scene::new(Options::default());
    s.load_at(two_castle_rows(), at(23, 30));
    s.enter(LORD, 40, vec![]);
    s.manor(
        LORD,
        ManorInput::SetSeed(RequestSetSeed {
            manor_id: 1,
            items: vec![seed_sale(SEED_1, 100, 9)],
        }),
    );
    let out = s.take();
    assert_eq!(action_failed(&out, LORD), 0);
    assert_eq!(s.world.manor_seed_product(1, SEED_1, true).unwrap().price, 7);
}

// ---------------------------------------------------------------------------------- the purchase

/// A farmer with `adena` next to the Gludio manager, who he has talked to; the period is APPROVED
/// and Gludio sells 40 seeds at 7 (a row of the current period).
fn buyer_scene(options: Options, adena_count: i64) -> (Scene, ObjectId) {
    let mut s = Scene::new(options);
    s.load_at(
        ManorLoad {
            production: vec![
                production(1, SEED_1, 40, 100, 7, false),
                production(1, SEED_2, 10, 10, 0, false),
            ],
            procure: Vec::new(),
        },
        at(12, 0),
    );
    s.enter(FARMER, 10, vec![adena(adena_count)]);
    let manager = s.npc(35644, 20);
    s.talk_to(FARMER, manager);
    s.take();
    (s, manager)
}

fn buy(manor_id: i32, items: &[(i32, i64)]) -> ManorInput {
    ManorInput::BuySeed(RequestBuySeed {
        manor_id,
        items: Some(
            items
                .iter()
                .map(|(item_id, count)| SeedPurchase {
                    item_id: *item_id,
                    count: *count,
                })
                .collect(),
        ),
    })
}

/// `RequestBuySeed`: the buyer pays `price * count`, gets the seeds, the castle's amount goes
/// down, and the adena goes to the treasury (`addToTreasuryNoTax`, a job of the switch).
#[test]
fn a_seed_purchase_pays_the_castle() {
    let (mut s, _) = buyer_scene(
        Options {
            siege_database_writes: true,
            ..Options::default()
        },
        1000,
    );
    s.manor(FARMER, buy(1, &[(SEED_1, 10)]));
    let out = s.take();
    assert_eq!(s.adena(FARMER), 1000 - 70);
    assert_eq!(s.count(FARMER, SEED_1), 10);
    assert_eq!(s.world.manor_seed_product(1, SEED_1, false).unwrap().amount, 30);
    assert_eq!(s.treasury(1), 1_000_000 + 70);
    assert!(
        residence_writes(&out).contains(&residence::ResidenceWrite::CastleTreasury {
            castle_id: 1,
            treasury: 1_000_070
        })
    );
    let told = messages_to(&out, FARMER);
    assert!(told.contains(
        &SystemMessage::new(SystemMessageId::S1AdenaDisappeared).with(SystemMessageParam::LongNumber(70))
    ));
    // `AltManorSaveAllActions` is off: no row of the manor.
    assert!(manor_writes(&out).is_empty());
}

#[test]
fn the_treasury_of_a_purchase_needs_the_switch() {
    let (mut s, _) = buyer_scene(Options::default(), 1000);
    s.manor(FARMER, buy(1, &[(SEED_1, 10)]));
    let out = s.take();
    assert_eq!(s.count(FARMER, SEED_1), 10);
    assert_eq!(s.treasury(1), 1_000_070, "this world's copy");
    assert!(residence_writes(&out).is_empty());
}

#[test]
fn a_purchase_with_save_all_actions_updates_the_amounts() {
    let (mut s, _) = buyer_scene(
        Options {
            manor: ManorSettings {
                save_all_actions: true,
                ..ManorSettings::default()
            },
            ..Options::default()
        },
        1000,
    );
    s.manor(FARMER, buy(1, &[(SEED_1, 10)]));
    assert_eq!(
        manor_writes(&s.take()),
        [ManorWrite::UpdateCurrentProduction {
            castle_id: 1,
            amounts: vec![(SEED_1, 30)]
        }]
    );
}

#[test]
fn a_purchase_that_cannot_happen_is_refused() {
    // More than the castle has (40): ActionFailed, nothing taken.
    let (mut s, _) = buyer_scene(Options::default(), 1000);
    s.manor(FARMER, buy(1, &[(SEED_1, 41)]));
    let out = s.take();
    assert_eq!(action_failed(&out, FARMER), 1);
    assert_eq!(s.adena(FARMER), 1000);
    // A seed with a price of 0 is not for sale.
    s.ticks(30);
    s.manor(FARMER, buy(1, &[(SEED_2, 1)]));
    assert_eq!(action_failed(&s.take(), FARMER), 1);
    // A seed the castle does not sell.
    s.ticks(30);
    s.manor(FARMER, buy(1, &[(9999, 1)]));
    assert_eq!(action_failed(&s.take(), FARMER), 1);
    // Not enough adena: the message, nothing taken.
    let (mut s, _) = buyer_scene(Options::default(), 50);
    s.manor(FARMER, buy(1, &[(SEED_1, 10)]));
    let out = s.take();
    assert_eq!(
        message_ids(&out, FARMER),
        [SystemMessageId::YouDoNotHaveEnoughAdena]
    );
    assert_eq!(s.adena(FARMER), 50);
    assert_eq!(s.world.manor_seed_product(1, SEED_1, false).unwrap().amount, 40);
    // A list Java would not read (`items: None`): ActionFailed.
    s.ticks(30);
    s.manor(
        FARMER,
        ManorInput::BuySeed(RequestBuySeed {
            manor_id: 1,
            items: None,
        }),
    );
    assert_eq!(action_failed(&s.take(), FARMER), 1);
}

#[test]
fn the_seller_must_be_the_manager_of_that_castle() {
    // Dion's manager does not sell Gludio's seeds.
    let (mut s, _) = buyer_scene(Options::default(), 1000);
    let dion = s.npc(35645, -20);
    s.talk_to(FARMER, dion);
    s.take();
    s.manor(FARMER, buy(1, &[(SEED_1, 1)]));
    assert_eq!(action_failed(&s.take(), FARMER), 1);
    assert_eq!(s.adena(FARMER), 1000);

    // An unknown castle.
    s.ticks(30);
    s.manor(FARMER, buy(77, &[(SEED_1, 1)]));
    assert_eq!(action_failed(&s.take(), FARMER), 1);
}

#[test]
fn nothing_is_bought_while_the_manor_is_in_maintenance() {
    let mut s = Scene::new(Options::default());
    s.load_at(
        ManorLoad {
            production: vec![production(1, SEED_1, 40, 100, 7, false)],
            procure: Vec::new(),
        },
        at(20, 3),
    );
    s.enter(FARMER, 10, vec![adena(1000)]);
    let manager = s.npc(35644, 20);
    s.talk_to(FARMER, manager);
    s.take();
    s.manor(FARMER, buy(1, &[(SEED_1, 1)]));
    assert_eq!(action_failed(&s.take(), FARMER), 1);
    assert_eq!(s.adena(FARMER), 1000);
}

// ---------------------------------------------------------------------------------- the sale

/// Gludio buys 30 more CROP_1 at 20 (reward type 1: item 1864, worth 10) and Dion 40 more CROP_2
/// at 30 (reward type 1: item 1865, worth 10). The farmer has 10 of each crop and 100 adena.
fn seller_scene(options: Options) -> (Scene, ObjectId) {
    seller_scene_with(
        options,
        vec![
            procure(1, CROP_1, 30, 50, 20, 1, false),
            procure(2, CROP_2, 40, 40, 30, 1, false),
        ],
    )
}

fn seller_scene_with(options: Options, rows: Vec<ManorProcureRow>) -> (Scene, ObjectId) {
    let mut s = Scene::new(options);
    s.load_at(
        ManorLoad {
            production: Vec::new(),
            procure: rows,
        },
        at(12, 0),
    );
    s.enter(
        FARMER,
        10,
        vec![
            adena(100),
            item(0x6000_2101, CROP_1, 10),
            item(0x6000_2102, CROP_2, 10),
        ],
    );
    let manager = s.npc(35644, 20);
    s.talk_to(FARMER, manager);
    s.take();
    (s, manager)
}

fn sell(items: &[(i32, i32, i32, i64)]) -> ManorInput {
    ManorInput::ProcureCropList(RequestProcureCropList {
        items: Some(
            items
                .iter()
                .map(|(object_id, item_id, manor_id, count)| CropSaleItem {
                    object_id: *object_id,
                    item_id: *item_id,
                    manor_id: *manor_id,
                    count: *count,
                })
                .collect(),
        ),
    })
}

/// `RequestProcureCropList` to the player's own manor: `price * count` of reward value, paid in
/// the reward item (`price / referencePrice`), no fee.
#[test]
fn a_crop_sold_to_the_own_manor_pays_in_reward_items() {
    let (mut s, _) = seller_scene(Options::default());
    s.manor(FARMER, sell(&[(0x6000_2101, CROP_1, 1, 10)]));
    // 10 crops at 20 are worth 200; the reward is worth 10 each: 20 of them.
    assert_eq!(s.count(FARMER, CROP_1), 0);
    assert_eq!(s.count(FARMER, REWARD_1A), 20);
    assert_eq!(s.adena(FARMER), 100, "no fee in the own manor");
    assert_eq!(
        s.world.manor_crop_procure_of(1, CROP_1, false).unwrap().amount,
        20
    );
}

/// To another castle's manor the seller pays a fee of 5% of the price (`(long) (price * 0.05)`).
#[test]
fn a_crop_sold_to_another_manor_costs_a_fee() {
    let (mut s, _) = seller_scene(Options::default());
    s.manor(FARMER, sell(&[(0x6000_2102, CROP_2, 2, 10)]));
    let out = s.take();
    // 10 crops at 30 are 300; the fee is 15 adena; the reward is 30 items of 1865.
    assert_eq!(s.adena(FARMER), 100 - 15);
    assert_eq!(s.count(FARMER, CROP_2), 0);
    assert_eq!(s.count(FARMER, 1865), 30);
    assert_eq!(
        s.world.manor_crop_procure_of(2, CROP_2, false).unwrap().amount,
        30
    );
    assert!(!messages_to(&out, FARMER).is_empty());
}

#[test]
fn a_seller_without_the_fee_keeps_the_crops() {
    // Dion pays 3000 a crop: the fee of 10 crops is 1500, the farmer has 100 adena.
    let (mut s, _) = seller_scene_with(
        Options::default(),
        vec![procure(2, CROP_2, 40, 40, 3_000, 1, false)],
    );
    s.manor(FARMER, sell(&[(0x6000_2102, CROP_2, 2, 10)]));
    let out = s.take();
    // 10 * 3000 = 30,000; the fee is 1500 > 100.
    assert_eq!(s.adena(FARMER), 100);
    assert_eq!(s.count(FARMER, CROP_2), 10);
    assert_eq!(
        message_ids(&out, FARMER),
        [
            SystemMessageId::FailedInTradingS2OfS1Crops,
            SystemMessageId::YouDoNotHaveEnoughAdena
        ]
    );
}

#[test]
fn a_sale_that_cannot_happen_is_refused() {
    let (mut s, _) = seller_scene(Options::default());
    // More than the manor wants (30), more than the player has (10), a wrong item id.
    for sale in [
        sell(&[(0x6000_2101, CROP_1, 1, 31)]),
        sell(&[(0x6000_2101, CROP_1, 1, 11)]),
        sell(&[(0x6000_2101, CROP_2, 1, 1)]),
        sell(&[(0x6000_2999, CROP_1, 1, 1)]),
    ] {
        s.ticks(30);
        s.take();
        s.manor(FARMER, sale);
        assert_eq!(action_failed(&s.take(), FARMER), 1);
    }
    assert_eq!(s.count(FARMER, CROP_1), 10);
    assert_eq!(
        s.world.manor_crop_procure_of(1, CROP_1, false).unwrap().amount,
        30
    );
}

#[test]
fn a_crop_worth_less_than_one_reward_item_is_refused_with_the_message() {
    let (mut s, _) = seller_scene_with(Options::default(), vec![procure(1, CROP_1, 30, 50, 5, 1, false)]);
    // 1 crop at 5 adena against a reward worth 10: `rewardItemCount < 1`.
    s.manor(FARMER, sell(&[(0x6000_2101, CROP_1, 1, 1)]));
    let out = s.take();
    let told = messages_to(&out, FARMER);
    assert_eq!(told.len(), 1);
    assert_eq!(told[0].id, SystemMessageId::FailedInTradingS2OfS1Crops);
    assert_eq!(s.count(FARMER, CROP_1), 10);
    assert_eq!(
        s.world.manor_crop_procure_of(1, CROP_1, false).unwrap().amount,
        30
    );
}

#[test]
fn a_sale_with_save_all_actions_updates_the_rows_of_the_managers_castle() {
    let (mut s, _) = seller_scene(Options {
        manor: ManorSettings {
            save_all_actions: true,
            ..ManorSettings::default()
        },
        ..Options::default()
    });
    s.manor(FARMER, sell(&[(0x6000_2101, CROP_1, 1, 10)]));
    assert_eq!(
        manor_writes(&s.take()),
        [ManorWrite::UpdateCurrentProcure {
            castle_id: 1,
            amounts: vec![(CROP_1, 20)]
        }]
    );
}

// ---------------------------------------------------------------------------------- the ManorManager

fn manager_rows() -> ManorLoad {
    ManorLoad {
        production: vec![
            production(1, SEED_1, 40, 100, 7, false),
            production(1, SEED_1, 60, 60, 8, true),
        ],
        procure: vec![
            procure(1, CROP_1, 30, 50, 20, 1, false),
            procure(1, CROP_1, 70, 70, 25, 2, true),
        ],
    }
}

/// A farmer with a crop and 100 adena next to the Gludio manager, who he has talked to, at `now`.
fn menu_scene(now: i64) -> (Scene, ObjectId) {
    let mut s = Scene::new(Options::default());
    s.load_at(manager_rows(), now);
    s.enter(FARMER, 10, vec![adena(100), item(0x6000_2101, CROP_1, 10)]);
    let manager = s.npc(35644, 20);
    s.talk_to(FARMER, manager);
    s.take();
    (s, manager)
}

fn bypass(s: &mut Scene, player: i32, command: &str) {
    s.ticks(30);
    s.take();
    s.input(
        player,
        ClientInput::Bypass {
            command: command.to_owned(),
        },
    );
}

/// `ManorManager.onFirstTalk`: `manager.htm`, and `manager-lord.htm` for the leader of the clan
/// that owns the castle of the manager (not for a GM).
#[test]
fn the_manager_shows_its_menu_and_the_lord_page() {
    let mut s = Scene::standard();
    s.enter(FARMER, 10, vec![]);
    s.enter(LORD, 40, vec![]);
    let manager = s.npc(35644, 20);
    s.talk_to(FARMER, manager);
    let page = html_to(&s.take(), FARMER);
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].npc_obj_id, manager.0);
    assert!(
        page[0]
            .html
            .starts_with(&format!("<html><body>Manor {} P3 ", manager.0)),
        "{}",
        page[0].html
    );

    s.talk_to(LORD, manager);
    let page = html_to(&s.take(), LORD);
    assert_eq!(page.len(), 1);
    assert_eq!(
        page[0].html,
        format!("<html><body>My Lord {}</body></html>", manager.0)
    );

    // The leader of another castle's clan gets the plain menu.
    s.enter(DION_LORD, 40, vec![]);
    s.talk_to(DION_LORD, manager);
    let page = html_to(&s.take(), DION_LORD);
    assert!(page[0].html.starts_with("<html><body>Manor "));
}

/// The link of the page opens a help page: `Script ManorManager <page>` is the script's `onEvent`
/// (`processScriptEvent`), answered with the page itself.
#[test]
fn the_help_pages_are_script_events() {
    let mut s = Scene::standard();
    s.enter(FARMER, 10, vec![]);
    let manager = s.npc(35644, 20);
    s.talk_to(FARMER, manager);
    s.take();
    bypass(&mut s, FARMER, "Script ManorManager manager-help-01.htm");
    let page = html_to(&s.take(), FARMER);
    assert_eq!(page.len(), 1);
    assert_eq!(
        page[0].html,
        format!("<html><body>Help 1 {}</body></html>", manager.0)
    );
}

/// Without the manor (`AllowManor = False`) the manager shows `npcdefault.htm`.
#[test]
fn a_disabled_manor_shows_the_default_page() {
    let mut s = Scene::new(Options {
        manor: ManorSettings {
            allow_manor: false,
            ..ManorSettings::default()
        },
        ..Options::default()
    });
    s.load_at(ManorLoad::default(), at(12, 0));
    s.enter(FARMER, 10, vec![]);
    let manager = s.npc(35644, 20);
    s.talk_to(FARMER, manager);
    let page = html_to(&s.take(), FARMER);
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].html, PAGE_DEFAULT);
}

#[test]
fn the_menu_opens_the_seed_purchase_and_refuses_another_castles_seeds() {
    let (mut s, _) = menu_scene(at(12, 0));
    bypass(&mut s, FARMER, "manor_menu_select?ask=1&state=-1&time=0");
    let lists = decoded::<BuyListSeed>(&s.take(), FARMER, "BUY_LIST_SEED");
    assert_eq!(
        lists,
        [BuyListSeed {
            money: 100,
            manor_id: 1,
            seeds: vec![SeedForSale {
                seed_id: SEED_1,
                amount: 40,
                price: 7
            }],
        }]
    );
    // `ask=1` for another castle's manor.
    bypass(&mut s, FARMER, "manor_menu_select?ask=1&state=2&time=0");
    let out = s.take();
    assert_eq!(
        messages_to(&out, FARMER),
        [
            SystemMessage::new(SystemMessageId::HereYouCanBuyOnlySeedsOfS1Manor)
                .with(SystemMessageParam::CastleName(1))
        ]
    );
}

#[test]
fn the_menu_lists_the_crops_the_player_can_sell() {
    let (mut s, _) = menu_scene(at(12, 0));
    bypass(&mut s, FARMER, "manor_menu_select?ask=2&state=-1&time=0");
    let lists = decoded::<ExShowSellCropList>(&s.take(), FARMER, "EX_SHOW_SELL_CROP_LIST");
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].manor_id, 1);
    assert_eq!(
        lists[0].items,
        [SellCropItem {
            object_id: 0x6000_2101,
            item_id: CROP_1,
            seed_level: 10,
            reward_1: REWARD_1A,
            reward_2: REWARD_1B,
            offer: Some(CropOffer {
                amount: 30,
                price: 20,
                reward: 1
            }),
            my_crops: 10,
        }]
    );
}

#[test]
fn the_menu_shows_the_seed_and_crop_info_of_both_periods() {
    // Approved: the next period is shown too.
    let (mut s, _) = menu_scene(at(12, 0));
    bypass(&mut s, FARMER, "manor_menu_select?ask=3&state=-1&time=0");
    let info = decoded::<ExShowSeedInfo>(&s.take(), FARMER, "EX_SHOW_SEED_INFO");
    assert_eq!(info.len(), 1);
    assert!(!info[0].hide_buttons);
    assert_eq!(info[0].manor_id, 1);
    assert_eq!(
        info[0].seeds,
        [SeedProductionInfo {
            seed_id: SEED_1,
            left: 40,
            started: 100,
            price: 7,
            level: 10,
            reward_1: REWARD_1A,
            reward_2: REWARD_1B,
        }]
    );
    bypass(&mut s, FARMER, "manor_menu_select?ask=3&state=-1&time=1");
    let info = decoded::<ExShowSeedInfo>(&s.take(), FARMER, "EX_SHOW_SEED_INFO");
    assert_eq!((info[0].seeds[0].started, info[0].seeds[0].price), (60, 8));
    bypass(&mut s, FARMER, "manor_menu_select?ask=4&state=-1&time=0");
    let info = decoded::<ExShowCropInfo>(&s.take(), FARMER, "EX_SHOW_CROP_INFO");
    assert_eq!(
        info[0].crops,
        [CropProcureInfo {
            crop_id: CROP_1,
            left: 30,
            started: 50,
            price: 20,
            reward: 1,
            level: 10,
            reward_1: REWARD_1A,
            reward_2: REWARD_1B,
        }]
    );

    // Modifiable: the next period is not shown before it is approved (Java sends no rows).
    let (mut s, _) = menu_scene(at(23, 30));
    bypass(&mut s, FARMER, "manor_menu_select?ask=3&state=-1&time=1");
    let info = decoded::<ExShowSeedInfo>(&s.take(), FARMER, "EX_SHOW_SEED_INFO");
    assert!(info[0].seeds.is_empty());
}

#[test]
fn the_menu_shows_the_default_crop_table() {
    let (mut s, _) = menu_scene(at(12, 0));
    bypass(&mut s, FARMER, "manor_menu_select?ask=5&state=-1&time=0");
    let info = decoded::<ExShowManorDefaultInfo>(&s.take(), FARMER, "EX_SHOW_MANOR_DEFAULT_INFO");
    assert_eq!(info.len(), 1);
    assert!(!info[0].hide_buttons);
    // The order of Java's `HashMap` of seeds (5016 before 5017).
    assert_eq!(
        info[0].crops,
        [
            ManorDefaultCrop {
                crop_id: CROP_1,
                level: 10,
                seed_price: 5,
                crop_price: 9,
                reward_1: REWARD_1A,
                reward_2: REWARD_1B,
            },
            ManorDefaultCrop {
                crop_id: CROP_2,
                level: 13,
                seed_price: 5,
                crop_price: 9,
                reward_1: 1865,
                reward_2: 1879,
            },
        ]
    );
}

#[test]
fn the_menu_lists_the_castles_that_buy_a_crop() {
    let mut s = Scene::new(Options::default());
    s.load_at(
        ManorLoad {
            production: Vec::new(),
            procure: vec![
                procure(2, CROP_1, 5, 5, 11, 2, false),
                procure(1, CROP_1, 30, 50, 20, 1, false),
                // Nothing left to buy in castle 3: not listed.
                procure(3, CROP_1, 0, 5, 11, 2, false),
            ],
        },
        at(12, 0),
    );
    s.enter(FARMER, 10, vec![]);
    let manager = s.npc(35644, 20);
    s.talk_to(FARMER, manager);
    bypass(
        &mut s,
        FARMER,
        &format!("manor_menu_select?ask=9&state={CROP_1}&time=0"),
    );
    let detail = decoded::<ExShowProcureCropDetail>(&s.take(), FARMER, "EX_SHOW_PROCURE_CROP_DETAIL");
    assert_eq!(
        detail,
        [ExShowProcureCropDetail {
            crop_id: CROP_1,
            castles: vec![
                ProcureCastle {
                    castle_id: 1,
                    amount: 30,
                    price: 20,
                    reward: 1
                },
                ProcureCastle {
                    castle_id: 2,
                    amount: 5,
                    price: 11,
                    reward: 2
                },
            ]
        }]
    );
}

#[test]
fn the_menu_is_closed_in_the_maintenance() {
    let (mut s, _) = menu_scene(at(20, 3));
    bypass(&mut s, FARMER, "manor_menu_select?ask=1&state=-1&time=0");
    let out = s.take();
    assert_eq!(
        message_ids(&out, FARMER),
        [SystemMessageId::TheManorSystemIsCurrentlyUnderMaintenance]
    );
    assert!(packets_to(&out, FARMER, "BUY_LIST_SEED").is_empty());
}

/// The same bypass at the castle chamberlain opens the owner's pages, without the purchase and sale
/// buttons; the settings only outside the approved period.
#[test]
fn the_chamberlain_opens_the_settings_for_the_owner() {
    let mut s = Scene::new(Options::default());
    s.load_at(manager_rows(), at(12, 0));
    s.enter(LORD, 40, vec![]);
    s.enter(FARMER, 10, vec![]);
    let chamberlain = s.npc(35100, 20);
    s.talk_to(LORD, chamberlain);
    s.talk_to(FARMER, chamberlain);
    s.take();

    // Approved: the settings are closed.
    bypass(&mut s, LORD, "manor_menu_select?ask=7&state=-1&time=0");
    assert_eq!(
        message_ids(&s.take(), LORD),
        [SystemMessageId::AManorCannotBeSetUpBetween430AmAnd8Pm]
    );
    bypass(&mut s, LORD, "manor_menu_select?ask=3&state=-1&time=0");
    let info = decoded::<ExShowSeedInfo>(&s.take(), LORD, "EX_SHOW_SEED_INFO");
    assert!(info[0].hide_buttons);
    // Somebody else's bypass answers nothing.
    bypass(&mut s, FARMER, "manor_menu_select?ask=3&state=-1&time=0");
    assert!(
        s.take()
            .iter()
            .all(|o| !matches!(o, Out::Packet(to, _) if *to == FARMER))
    );

    // Modifiable: the seed and crop settings of the castle.
    s.world.manor_change_mode_at(at(20, 0), utc);
    s.world.manor_change_mode_at(at(20, 6), utc);
    assert!(s.world.manor_is_modifiable());
    bypass(&mut s, LORD, "manor_menu_select?ask=7&state=-1&time=0");
    let settings = decoded::<ExShowSeedSetting>(&s.take(), LORD, "EX_SHOW_SEED_SETTING");
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0].manor_id, 1);
    let seed = &settings[0].seeds[0];
    assert_eq!(
        (
            seed.seed_id,
            seed.level,
            seed.sale_limit,
            seed.reference_price,
            seed.min_price,
            seed.max_price
        ),
        (SEED_1, 10, 8100, 5, 3, 50)
    );
    bypass(&mut s, LORD, "manor_menu_select?ask=8&state=-1&time=0");
    let settings = decoded::<ExShowCropSetting>(&s.take(), LORD, "EX_SHOW_CROP_SETTING");
    let crop = &settings[0].crops[0];
    assert_eq!(
        (
            crop.crop_id,
            crop.level,
            crop.sale_limit,
            crop.min_price,
            crop.max_price
        ),
        (CROP_1, 10, 9000, 5, 90)
    );
}

// ---------------------------------------------------------------------------------- sowing

const SEED_OBJECT: i32 = 0x6000_2200;
const HARVESTER_OBJECT: i32 = 0x6000_2201;

fn farm_scene(seed: u64) -> Scene {
    let mut s = Scene::new(Options {
        seed,
        ..Options::default()
    });
    s.load_at(ManorLoad::default(), at(12, 0));
    s
}

fn use_item(s: &mut Scene, player: i32, object_id: i32) {
    s.ticks(30);
    s.input(
        player,
        ClientInput::UseItem {
            object_id,
            ctrl_pressed: false,
        },
    );
}

fn farmer_items() -> Vec<ItemData> {
    vec![
        item(SEED_OBJECT, SEED_1, 5),
        item(HARVESTER_OBJECT, HARVESTER, 1),
        adena(100),
    ]
}

#[test]
fn a_seed_needs_a_monster_to_sow() {
    let mut s = farm_scene(1);
    s.enter(FARMER, 10, farmer_items());
    // No target.
    use_item(&mut s, FARMER, SEED_OBJECT);
    assert_eq!(message_ids(&s.take(), FARMER), [SystemMessageId::InvalidTarget]);
    // A manager is an NPC but no monster.
    let manager = s.npc(35644, 20);
    s.world.set_target(ObjectId(FARMER), Some(manager));
    use_item(&mut s, FARMER, SEED_OBJECT);
    assert_eq!(
        message_ids(&s.take(), FARMER),
        [SystemMessageId::TheTargetIsUnavailableForSeeding]
    );
    // A raid boss and a chest are refused the same way.
    for npc_id in [9102, 9103] {
        let target = s.npc(npc_id, 30);
        s.world.set_target(ObjectId(FARMER), Some(target));
        use_item(&mut s, FARMER, SEED_OBJECT);
        assert_eq!(
            message_ids(&s.take(), FARMER),
            [SystemMessageId::TheTargetIsUnavailableForSeeding],
            "{npc_id}"
        );
    }
    assert_eq!(s.count(FARMER, SEED_1), 5, "no seed was used");
}

#[test]
fn a_dead_monster_cannot_be_sown() {
    let mut s = farm_scene(1);
    s.enter(FARMER, 10, farmer_items());
    let monster = s.npc(9100, 10);
    s.world
        .reduce_current_hp(monster, 1_000_000.0, Some(ObjectId(FARMER)));
    s.world.set_target(ObjectId(FARMER), Some(monster));
    use_item(&mut s, FARMER, SEED_OBJECT);
    assert_eq!(message_ids(&s.take(), FARMER), [SystemMessageId::InvalidTarget]);
}

/// A seed of Gludio is sown only in Gludio's area (`MapRegionData.getAreaCastle`).
#[test]
fn a_seed_is_sown_only_in_its_castles_area() {
    let mut s = farm_scene(1);
    // Tile 16, the area of Dion (castle 2).
    s.enter_at(FARMER, 10, farmer_items(), (-120_000, Y, Z));
    let monster = s.world.spawn_npc_at(9100, -119_990, Y, Z, 0).unwrap();
    s.world.set_target(ObjectId(FARMER), Some(monster));
    use_item(&mut s, FARMER, SEED_OBJECT);
    assert_eq!(
        message_ids(&s.take(), FARMER),
        [SystemMessageId::ThisSeedMayNotBeSownHere]
    );
    assert!(s.world.seeder_of(monster).is_none());
}

/// Using a seed on a monster stores the seed and the seeder and casts Sowing; the Sow effect takes
/// the seed and rolls the chance (`Rnd.get(99) < 90` here). Over many worlds both outcomes
/// happen, each with its message.
#[test]
fn sowing_consumes_the_seed_and_sometimes_seeds_the_monster() {
    let mut seeded = 0;
    let mut failed = 0;
    for seed in 0..40_u64 {
        let mut s = farm_scene(seed);
        s.enter(FARMER, 10, farmer_items());
        let monster = s.npc(9100, 10);
        s.world.set_target(ObjectId(FARMER), Some(monster));
        use_item(&mut s, FARMER, SEED_OBJECT);
        // The seed and the seeder are stored at once, the monster is not seeded yet.
        assert_eq!(s.world.seeder_of(monster), Some(ObjectId(FARMER)));
        assert_eq!(s.world.seed_of(monster), Some(SEED_1));
        assert!(!s.world.is_seeded(monster));
        s.ticks(60);
        let told = message_ids(&s.take(), FARMER);
        // The seed is used up whatever the roll says (`destroyItemByItemId(.., 1, ..)`).
        assert_eq!(s.count(FARMER, SEED_1), 4, "seed {seed}");
        if s.world.is_seeded(monster) {
            seeded += 1;
            assert!(
                told.contains(&SystemMessageId::TheSeedWasSuccessfullySown),
                "{told:?}"
            );
        } else {
            failed += 1;
            assert!(told.contains(&SystemMessageId::TheSeedWasNotSown), "{told:?}");
        }
    }
    assert!(seeded > 20 && failed >= 1, "seeded {seeded}, failed {failed}");
}

// ---------------------------------------------------------------------------------- harvesting

/// Sows (with the RNG seeds 0, 1, 2, ...) until the sowing has the wanted outcome; the seeder is
/// the farmer. Returns the scene and the monster, the messages cleared.
fn sow_until(player_level: i32, monster_npc: i32, manor: ManorSettings, seeded: bool) -> (Scene, ObjectId) {
    for seed in 0..400_u64 {
        let mut s = Scene::new(Options {
            seed,
            manor: manor.clone(),
            ..Options::default()
        });
        s.load_at(ManorLoad::default(), at(12, 0));
        s.enter(FARMER, player_level, farmer_items());
        let monster = s.npc(monster_npc, 10);
        s.world.set_target(ObjectId(FARMER), Some(monster));
        use_item(&mut s, FARMER, SEED_OBJECT);
        s.ticks(60);
        if s.world.is_seeded(monster) == seeded {
            s.take();
            return (s, monster);
        }
    }
    panic!("no RNG seed gave the wanted sowing");
}

fn kill(s: &mut Scene, monster: ObjectId, killer: i32) {
    s.world
        .reduce_current_hp(monster, 1_000_000.0, Some(ObjectId(killer)));
    s.ticks(2);
    s.take();
}

/// Harvester + Harvesting: the seeder takes the crop of the dead monster once: one crop for a
/// monster of the seed's level (`count = 1`, no bonus), `RateDropManor` times.
#[test]
fn the_seeder_harvests_the_crop_once() {
    let (mut s, monster) = sow_until(10, 9100, ManorSettings::default(), true);
    kill(&mut s, monster, FARMER);
    s.world.set_target(ObjectId(FARMER), Some(monster));
    use_item(&mut s, FARMER, HARVESTER_OBJECT);
    s.ticks(40);
    let out = s.take();
    assert_eq!(s.count(FARMER, CROP_1), 1);
    assert_eq!(message_ids(&out, FARMER).first(), Some(&SystemMessageId::UseS1));
    assert!(message_ids(&out, FARMER).contains(&SystemMessageId::YouHaveObtainedS1));

    // The crop is taken: a second harvest gives nothing.
    use_item(&mut s, FARMER, HARVESTER_OBJECT);
    s.ticks(40);
    assert_eq!(s.count(FARMER, CROP_1), 1);
}

#[test]
fn the_crop_follows_the_rate_and_the_monsters_strength() {
    // `RateDropManor = 3` on a plain monster.
    let (mut s, monster) = sow_until(
        10,
        9100,
        ManorSettings {
            rate_drop_manor: 3,
            ..ManorSettings::default()
        },
        true,
    );
    kill(&mut s, monster, FARMER);
    s.world.set_target(ObjectId(FARMER), Some(monster));
    use_item(&mut s, FARMER, HARVESTER_OBJECT);
    s.ticks(40);
    assert_eq!(s.count(FARMER, CROP_1), 3);
    let out = s.take();
    let told = messages_to(&out, FARMER);
    assert!(
        told.contains(
            &SystemMessage::new(SystemMessageId::YouHaveObtainedS2S1)
                .with(SystemMessageParam::ItemName(CROP_1))
                .with(SystemMessageParam::LongNumber(3))
        )
    );

    // A level 30 monster with the "strong type x4" skill 4305: 1 * 4, plus 30 - 10 - 5 levels
    // above the seed's: 19 crops.
    let (mut s, monster) = sow_until(30, 9104, ManorSettings::default(), true);
    kill(&mut s, monster, FARMER);
    s.world.set_target(ObjectId(FARMER), Some(monster));
    use_item(&mut s, FARMER, HARVESTER_OBJECT);
    s.ticks(40);
    assert_eq!(s.count(FARMER, CROP_1), 19);
}

/// Only the seeder may harvest; a harvest of a monster whose sowing failed says so.
#[test]
fn nobody_else_harvests_and_an_unseeded_monster_gives_nothing() {
    let (mut s, monster) = sow_until(10, 9100, ManorSettings::default(), true);
    s.enter(MEMBER, 10, vec![]);
    kill(&mut s, monster, FARMER);
    s.world
        .apply_effects(ObjectId(MEMBER), monster, SkillKey::new(2098, 1));
    assert_eq!(
        message_ids(&s.take(), MEMBER),
        [SystemMessageId::YouAreNotAuthorizedToHarvest]
    );
    assert_eq!(s.count(MEMBER, CROP_1), 0);

    // The sowing failed: the seeder is the farmer but the monster is not seeded.
    let (mut s, monster) = sow_until(10, 9100, ManorSettings::default(), false);
    kill(&mut s, monster, FARMER);
    s.world
        .apply_effects(ObjectId(FARMER), monster, SkillKey::new(2098, 1));
    assert_eq!(
        message_ids(&s.take(), FARMER),
        [SystemMessageId::TheHarvestFailedBecauseTheSeedWasNotSown]
    );
}

#[test]
fn the_harvester_needs_a_dead_monster() {
    let mut s = farm_scene(1);
    s.enter(FARMER, 10, farmer_items());
    // No target, then a live monster.
    use_item(&mut s, FARMER, HARVESTER_OBJECT);
    let out = s.take();
    assert_eq!(message_ids(&out, FARMER), [SystemMessageId::InvalidTarget]);
    assert_eq!(action_failed(&out, FARMER), 1);
    let monster = s.npc(9100, 10);
    s.world.set_target(ObjectId(FARMER), Some(monster));
    use_item(&mut s, FARMER, HARVESTER_OBJECT);
    assert_eq!(message_ids(&s.take(), FARMER), [SystemMessageId::InvalidTarget]);
}

/// The harvest chance falls 5% for every level between the player and the monster past 5.
#[test]
fn a_harvest_can_fail() {
    // The player is 20 levels above the monster: 100 - (20 - 5) * 5 = 25%.
    let mut failed = 0;
    let mut gained = 0;
    for seed in 0..60_u64 {
        let mut s = Scene::new(Options {
            seed,
            ..Options::default()
        });
        s.load_at(ManorLoad::default(), at(12, 0));
        s.enter(FARMER, 30, farmer_items());
        let monster = s.npc(9100, 10);
        s.world.set_target(ObjectId(FARMER), Some(monster));
        use_item(&mut s, FARMER, SEED_OBJECT);
        s.ticks(60);
        if !s.world.is_seeded(monster) {
            continue;
        }
        kill(&mut s, monster, FARMER);
        s.world
            .apply_effects(ObjectId(FARMER), monster, SkillKey::new(2098, 1));
        let told = message_ids(&s.take(), FARMER);
        if told.contains(&SystemMessageId::TheHarvestHasFailed) {
            failed += 1;
        } else {
            assert_eq!(s.count(FARMER, CROP_1), 1);
            gained += 1;
        }
    }
    assert!(failed > 0 && gained > 0, "failed {failed}, gained {gained}");
}

/// `NpcTemplate.calculateDrops`: a seeded monster drops only adena and the seal stones; the
/// corpse stays `SpoiledCorpseExtendTime` longer; the state goes with the corpse.
#[test]
fn a_seeded_monster_drops_only_adena_and_keeps_its_corpse_longer() {
    let (mut s, monster) = sow_until(10, 9100, ManorSettings::default(), true);
    kill(&mut s, monster, FARMER);
    let dropped: Vec<i32> = s
        .world
        .ground_item_ids()
        .into_iter()
        .map(|id| s.world.ground_item(id).unwrap().data.item_id)
        .collect();
    assert_eq!(dropped, [ADENA]);
    let seeded_corpse = s.world.decay_remaining_ms(monster);

    // The same monster unseeded drops its second item too.
    let mut plain = farm_scene(5);
    plain.enter(FARMER, 10, farmer_items());
    let other = plain.npc(9100, 10);
    kill(&mut plain, other, FARMER);
    let mut dropped: Vec<i32> = plain
        .world
        .ground_item_ids()
        .into_iter()
        .map(|id| plain.world.ground_item(id).unwrap().data.item_id)
        .collect();
    dropped.sort_unstable();
    assert_eq!(dropped, [ADENA, REWARD_1A]);
    // `SPOILED_CORPSE_EXTEND_TIME` (10 s) more than the plain corpse.
    let plain_corpse = plain.world.decay_remaining_ms(other);
    assert!(
        (seeded_corpse - plain_corpse - 10_000).abs() <= 200,
        "{seeded_corpse} against {plain_corpse}"
    );

    // The corpse decays; the seeded state is gone with it.
    s.ticks(600);
    assert!(!s.world.is_seeded(monster));
    assert!(s.world.seeder_of(monster).is_none());
}

/// The monster has the seed only after the handler stored it, and a monster already sown refuses
/// another seed with an `ActionFailed`.
#[test]
fn a_sown_monster_refuses_another_seed() {
    let (mut s, monster) = sow_until(10, 9100, ManorSettings::default(), true);
    s.world.set_target(ObjectId(FARMER), Some(monster));
    use_item(&mut s, FARMER, SEED_OBJECT);
    let out = s.take();
    assert_eq!(action_failed(&out, FARMER), 1);
    // Nothing is cast ("Use S1") and nothing is sown.
    assert!(!message_ids(&out, FARMER).contains(&SystemMessageId::UseS1));
    assert_eq!(s.count(FARMER, SEED_1), 4, "the second seed was not used");
}

// ---------------------------------------------------------------------------------- the rest

/// The crops join the stack the clan warehouse already has (`ItemContainer.addItem` of a
/// stackable item): the row is updated, not inserted.
#[test]
fn the_crops_join_the_stack_in_the_warehouse() {
    let mut s = Scene::new(Options {
        siege_database_writes: true,
        lions_warehouse: vec![ItemData {
            loc: "CLANWH".into(),
            ..item(0x6000_1000, MATURE_1, 5)
        }],
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(19, 59) + 59_000);
    s.world.manor_change_mode_at(at(20, 0), utc);
    let out = s.take();
    assert_eq!(s.warehouse_count(LIONS, MATURE_1), 5 + 18);
    assert!(
        item_ops(&out)
            .iter()
            .any(|op| matches!(op, ItemRowOp::Update { owner_id, item }
            if *owner_id == LIONS && item.object_id == ObjectId(0x6000_1000) && item.count == 23)),
        "{:?}",
        item_ops(&out)
    );
}

/// `harvestedAmount < 1`: 90% (`Rnd.get(99) < 90`) of the castles that bought a crop but less than
/// 2 of it still get one mature crop. The world's RNG decides; both outcomes happen.
#[test]
fn a_single_crop_bought_is_a_gamble() {
    let (mut got, mut missed) = (0, 0);
    for seed in 0..40_u64 {
        let mut s = Scene::new(Options {
            siege_database_writes: true,
            seed,
            ..Options::default()
        });
        s.load_at(
            ManorLoad {
                production: Vec::new(),
                // One wanted, one bought: `(long) (1 * 0.9)` is 0.
                procure: vec![procure(1, CROP_1, 0, 1, 9, 1, false)],
            },
            at(19, 59) + 59_000,
        );
        s.world.manor_change_mode_at(at(20, 0), utc);
        match s.warehouse_count(LIONS, MATURE_1) {
            0 => missed += 1,
            1 => got += 1,
            other => panic!("{other} crops"),
        }
    }
    assert!(got > 25 && missed >= 1, "got {got}, missed {missed}");
}

/// `scheduleAtFixedRate(this::storeMe, rate hours)`: every `AltManorSavePeriodRate` hours the
/// tables are written (only with the switch), unless every action is saved at once.
#[test]
fn the_tables_are_saved_every_two_hours() {
    let two_hours = 144_000;
    let mut s = Scene::new(Options {
        siege_database_writes: true,
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(12, 0));
    s.ticks(two_hours - 10);
    assert!(manor_writes(&s.take()).is_empty());
    s.ticks(20);
    assert_eq!(manor_writes(&s.take()).len(), 1);
    s.ticks(two_hours);
    assert_eq!(manor_writes(&s.take()).len(), 1, "and again two hours later");

    // Without the switch Sieghardt's rows stay.
    let mut s = Scene::new(Options::default());
    s.load_at(two_castle_rows(), at(12, 0));
    s.ticks(two_hours + 10);
    assert!(manor_writes(&s.take()).is_empty());

    // With every action saved there is no timer.
    let mut s = Scene::new(Options {
        siege_database_writes: true,
        manor: ManorSettings {
            save_all_actions: true,
            ..ManorSettings::default()
        },
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(12, 0));
    s.ticks(two_hours + 10);
    assert!(manor_writes(&s.take()).is_empty());
}

/// `Shutdown`: "Save all manor data." unless every action was saved.
#[test]
fn the_shutdown_saves_the_manor() {
    let shutdown = |s: &mut Scene| {
        s.world.handle(WorldCommand::Shutdown {
            done: Box::new(|| {}),
        });
        s.take()
    };
    let mut s = Scene::new(Options {
        siege_database_writes: true,
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(12, 0));
    assert_eq!(manor_writes(&shutdown(&mut s)).len(), 1);

    let mut s = Scene::new(Options {
        siege_database_writes: true,
        manor: ManorSettings {
            save_all_actions: true,
            ..ManorSettings::default()
        },
        ..Options::default()
    });
    s.load_at(two_castle_rows(), at(12, 0));
    assert!(manor_writes(&shutdown(&mut s)).is_empty());

    let mut s = Scene::new(Options::default());
    s.load_at(two_castle_rows(), at(12, 0));
    assert!(manor_writes(&shutdown(&mut s)).is_empty());
}
