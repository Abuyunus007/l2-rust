#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Mounts (M6-FU-07): action 38 and the checks of `Player.mountPlayer`, `mount`/`dismount`, the
//! `Ride` packet, the speeds of the ride, the food of the mount and the hunger, the wyvern and its
//! no-landing zones, the `//ride_*` admin commands' mount and the rows (`pets.fed`). Expected
//! values follow the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, ItemTemplates, KarmaLoss, MapRegions,
    NpcLoadOptions, NpcTemplates, PetDataTable, PetSkillData, PlayerTemplates, SkillTemplates, SkillTrees,
    SpawnData, Zones,
};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::{PetDelete, Ride, SetupGauge, SkillList, SystemMessage, UserInfo};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::mount::{MountType, WYVERN_BREATH_ID};
use l2_world::stats::{BaseStatBonus, MoveType, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

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

#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Outbox for Persisted {
    fn persist(&mut self, command: PersistCommand) {
        self.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
    /// The summon table writes since the last call.
    fn summon_writes(&self) -> Vec<SummonWrite> {
        let mut commands = self.0.lock().unwrap();
        let writes = commands
            .iter()
            .filter_map(|c| match c {
                PersistCommand::Summon(w) => Some(w.clone()),
                _ => None,
            })
            .collect();
        commands.retain(|c| !matches!(c, PersistCommand::Summon(_)));
        writes
    }
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
</staticData><lvlUpgainData><level val="1"><hp>1000</hp><mp>500</mp><cp>200</cp></level></lvlUpgainData></list>"#;

/// 2046 the shape of the datapack's collar skill (`SummonPet`), 2048 of a food (`feed` 100), 4289
/// of Wyvern Breath, 3050 a debuff (`effectPoint` below 0 against a target) and 3051 a toggle.
const SKILLS: &str = r##"<list>
<skill id="2046" levels="2" name="Wolf's Necklace">
	<operateType>A1</operateType>
	<targetType>SELF</targetType>
	<hitTime>500</hitTime>
	<isMagic>2</isMagic>
	<magicLevel>1</magicLevel>
	<effects><effect name="SummonPet" /></effects>
</skill>
<skill id="2048" levels="1" name="Wolves' Food">
	<operateType>A1</operateType>
	<targetType>SELF</targetType>
	<feed>100</feed>
	<isMagic>2</isMagic>
	<magicLevel>1</magicLevel>
</skill>
<skill id="4289" levels="1" name="Wyvern Breath">
	<operateType>A1</operateType>
	<targetType>ONE</targetType>
	<castRange>600</castRange>
	<effectPoint>-100</effectPoint>
	<isMagic>1</isMagic>
	<magicLevel>1</magicLevel>
</skill>
<skill id="3050" levels="1" name="Test Debuff">
	<operateType>A1</operateType>
	<targetType>ONE</targetType>
	<castRange>600</castRange>
	<effectPoint>-100</effectPoint>
	<isMagic>1</isMagic>
	<magicLevel>1</magicLevel>
</skill>
<skill id="49991" levels="1" name="Twilight Dragon">
	<operateType>A1</operateType>
	<targetType>SELF</targetType>
	<isMagic>2</isMagic>
	<magicLevel>1</magicLevel>
	<effects><effect name="Ride"><npcId>12526</npcId></effect></effects>
</skill>
<skill id="3060" levels="1" name="Ride Test">
	<operateType>A1</operateType>
	<targetType>SELF</targetType>
	<isMagic>2</isMagic>
	<magicLevel>1</magicLevel>
	<effects><effect name="Ride"><npcId>12526</npcId></effect></effects>
</skill>
</list>"##;

const STRIDER_COLLAR: i32 = 4422;
const FOOD: i32 = 7582;
const SWORD: i32 = 9101;

const ITEMS: &str = r##"<list>
<item id="4422" type="EtcItem" name="Wind Strider Saddle">
	<set name="default_action" val="SKILL_MAINTAIN" /><set name="etcitem_type" val="PET_COLLAR" />
	<set name="weight" val="20" /><set name="handler" val="SummonItems" />
	<skills><skill id="2046" level="1" /></skills>
</item>
<item id="7582" type="EtcItem" name="Strider Food">
	<set name="is_stackable" val="true" /><set name="for_npc" val="true" /><set name="weight" val="1" />
	<set name="handler" val="PetFood" />
	<skills><skill id="2048" level="1" /></skills>
</item>
<item id="9819" type="EtcItem" name="Fortress Combat Flag">
	<set name="weight" val="0" />
</item>
<item id="9101" type="Weapon" name="Test Sword">
	<set name="weight" val="30" /><set name="bodypart" val="rhand" /><set name="weapon_type" val="SWORD" />
</item>
</list>"##;

/// 12526 a strider and 12621 the wyvern, both `Pet` NPCs; 9001 a monster.
const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="12526" level="55" type="Pet" name="Wind Strider">
    <race>DRAGON</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="25">
      <vitals hp="460.96247" hpRegen="3.5" mp="259.2" mpRegen="1.5" />
      <attack physical="64.83203" magical="44.27187" random="10" critical="4" accuracy="4.75" attackSpeed="253" type="FIST" range="40" distance="80" width="120" />
      <defence physical="101.83533" magical="74.51868" />
      <speed><walk ground="30" /><run ground="120" /></speed>
    </stats>
    <collision><radius normal="19"/><height normal="26"/></collision>
  </npc>
<npc id="12621" level="70" type="Pet" name="Wyvern">
    <race>DRAGON</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="25">
      <vitals hp="460.96247" hpRegen="3.5" mp="259.2" mpRegen="1.5" />
      <attack physical="64.83203" magical="44.27187" random="10" critical="4" accuracy="4.75" attackSpeed="253" type="FIST" range="40" distance="80" width="120" />
      <defence physical="101.83533" magical="74.51868" />
      <speed><walk ground="30" /><run ground="120" /></speed>
    </stats>
    <collision><radius normal="40"/><height normal="30"/></collision>
  </npc>
<npc id="9001" type="Monster" level="1" name="Gremlin">
    <race>FAIRY</race>
    <acquire exp="1000" sp="100" />
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc></list>"#;

/// One level of the pet data, with the ride speeds of the datapack's strider (`walk=run=130`).
fn pet_level(level: i32, exp: i64, ride: &str) -> String {
    format!(
        r#"<stat level="{level}"><set name="max_meal" val="100"/><set name="exp" val="{exp}"/>
        <set name="get_exp_type" val="70"/><set name="consume_meal_in_battle" val="13"/>
        <set name="consume_meal_in_normal" val="2"/><set name="org_pattack" val="10"/>
        <set name="org_pdefend" val="20"/><set name="org_mattack" val="15"/><set name="org_mdefend" val="25"/>
        <set name="org_hp" val="200"/><set name="org_mp" val="80"/><set name="org_hp_regen" val="6"/>
        <set name="org_mp_regen" val="3"/><set name="soulshot_count" val="2"/>
        <set name="spiritshot_count" val="1"/>{ride}</stat>"#
    )
}

/// The strider has levels 30 to 32 (a pet of level 30 is ten levels above the test player), the
/// wyvern the level of the test player (20).
fn pet_data() -> Arc<PetDataTable> {
    let strider: String = (30..=32)
        .map(|l| {
            pet_level(
                l,
                i64::from(l - 30) * 100,
                r#"<set name="speed_on_ride" val="0" walk="130" run="140" slowSwim="70" fastSwim="75"/>"#,
            )
        })
        .collect();
    let wyvern: String = [20]
        .into_iter()
        .map(|l| {
            pet_level(
                l,
                0,
                r#"<set name="speed_on_ride" val="0" walk="250" run="260" slowSwim="140" fastSwim="145" slowFly="250" fastFly="270"/>"#,
            )
        })
        .collect();
    Arc::new(
        PetDataTable::from_xml(&format!(
            r#"<pets>
            <pet id="12526" itemId="4422">
              <set name="food" val="7582"/><set name="hungry_limit" val="55"/><set name="load" val="54510"/>
              <stats>{strider}</stats></pet>
            <pet id="12621" itemId="-1">
              <set name="food" val="7582"/><set name="hungry_limit" val="55"/><set name="load" val="54510"/>
              <stats>{wyvern}</stats></pet>
            </pets>"#
        ))
        .unwrap(),
    )
}

#[derive(Default)]
struct Data {
    stat: Option<StatData>,
    gm: bool,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let template = self.stat.as_ref()?.items.get(item_id)?;
        Some(ItemStatic {
            display_id: template.display_id,
            type2: template.type2 as i16,
            body_part: 0,
            quest_item: template.quest_item,
            weight: template.weight,
            shared_reuse_group: template.shared_reuse_group,
            weapon: template.is_weapon(),
        })
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.stat
            .as_ref()
            .and_then(|s| s.items.get(item_id))
            .is_some_and(|t| t.stackable)
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        if self.gm {
            AccessLevelStatic {
                is_gm: true,
                ..AccessLevelStatic::USER
            }
        } else {
            AccessLevelStatic::USER
        }
    }
    fn stat_data(&self) -> Option<&StatData> {
        self.stat.as_ref()
    }
}

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    }
}

const EXP: [i64; 30] = [
    0, 68, 364, 1169, 2799, 5689, 10521, 18149, 29775, 46645, 70581, 103707, 148453, 207883, 284889, 382585,
    504265, 653421, 833889, 1050163, 1306997, 1609583, 1963577, 2374935, 2849807, 3394753, 4016663, 4722783,
    5520693, 6418300,
];

fn experience() -> Arc<ExperienceTable> {
    let rows: String = EXP
        .iter()
        .enumerate()
        .map(|(i, exp)| format!(r#"<experience level="{}" tolevel="{exp}" />"#, i + 1))
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="29" maxPetLevel="29">{rows}</table>"#
        ))
        .unwrap(),
    )
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const CONTROL: ObjectId = ObjectId(0x2100_0001);
const FOOD_STACK: ObjectId = ObjectId(0x2100_0002);
const SWORD_ID: ObjectId = ObjectId(0x2100_0003);

fn item(object_id: ObjectId, item_id: i32, count: i64, loc: &str) -> ItemData {
    ItemData {
        object_id,
        item_id,
        count,
        loc: loc.into(),
        ..Default::default()
    }
}

fn player_data(items: Vec<ItemData>, profile: PlayerProfile) -> PlayerData {
    PlayerData {
        object_id: ME,
        account: "acc".into(),
        name: "Rider".into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 20,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 5,
        access_level: 0,
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 1000.0,
        max_hp: 1000.0,
        cur_mp: 500.0,
        max_mp: 500.0,
        cur_cp: 200.0,
        max_cp: 200.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile,
    }
}

/// The `pets` row of the strider collar: level 30, `fed` as asked.
fn strider_row(fed: i32) -> SavedPet {
    SavedPet {
        control_object_id: CONTROL,
        name: None,
        level: 30,
        cur_hp: 200,
        cur_mp: 80,
        exp: 0,
        sp: 0,
        fed,
        restore: false,
    }
}

struct Scene {
    world: World,
    me: Log,
    persisted: Persisted,
}

impl Scene {
    fn new(items: Vec<ItemData>, fed: Option<i32>, gm: bool) -> Self {
        Self::build(items, fed, gm, (false, false))
    }

    /// `free_mounts`: `EnableFreeStrider` and `EnableFreeWyvern` of `Custom/FreeMounts.ini`.
    fn build(items: Vec<ItemData>, fed: Option<i32>, gm: bool, free_mounts: (bool, bool)) -> Self {
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(persisted.clone()));
        world.set_static_data(Box::new(Data {
            stat: Some(stat_data()),
            gm,
        }));
        let mut settings = WorldSettings::default();
        settings.exp.enable_free_strider = free_mounts.0;
        settings.exp.enable_free_wyvern = free_mounts.1;
        world.set_settings(settings);
        world.set_regen_settings(RegenSettings {
            hp_regen_multiplier: 0.0,
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        world.set_exp_data(
            experience(),
            Arc::new(ExperienceLoss::default()),
            Arc::new(KarmaLoss::default()),
            Arc::new(SkillTrees::default()),
        );
        world.set_pet_skill_data(Arc::new(
            PetSkillData::from_xml("<list></list>", &|_, _| true).unwrap(),
        ));
        world.set_pet_data(pet_data());
        world.set_pet_settings(PetSettings {
            player_spawn_protection: 600,
            ..PetSettings::default()
        });
        let profile = PlayerProfile {
            pet_rows: fed.map(strider_row).into_iter().collect(),
            ..PlayerProfile::default()
        };
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(items, profile)),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let mut scene = Self { world, me, persisted };
        scene.advance(1000);
        scene.me.take();
        scene.persisted.summon_writes();
        scene
    }

    /// A rider with the saddle of a strider that is fed `fed`.
    fn with_strider(fed: i32) -> Self {
        Self::new(
            vec![item(CONTROL, STRIDER_COLLAR, 1, "INVENTORY")],
            Some(fed),
            false,
        )
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn input(&mut self, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input,
        });
    }

    fn action(&mut self, action_id: i32) {
        self.input(ClientInput::ActionUse {
            action_id,
            ctrl_pressed: false,
            shift_pressed: false,
        });
    }

    /// The saddle double clicked, the pet is out.
    fn summon_strider(&mut self) -> ObjectId {
        self.input(ClientInput::UseItem {
            object_id: CONTROL.0,
            ctrl_pressed: false,
        });
        self.advance(1000);
        self.world.summon_of(ME).expect("a strider")
    }

    /// The strider out and mounted by action 38.
    fn mounted_strider(&mut self) -> ObjectId {
        let pet = self.summon_strider();
        self.me.take();
        self.action(38);
        pet
    }

    fn view_run_speed(&self) -> f64 {
        self.world.stats(ME).unwrap().run_speed()
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

fn of<'a>(packets: &'a [Vec<u8>], name: &str) -> Vec<&'a Vec<u8>> {
    packets.iter().filter(|p| name_of(p) == name).collect()
}

fn message_ids(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    of(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p).id)
        .collect()
}

// ---- the types ---------------------------------------------------------------------------------

/// `MountType.findByNpcId` by `categoryData.xml`, and the ordinals of `UserInfo` and `Ride`.
#[test]
fn mount_types_follow_the_categories() {
    for strider in [12526, 12527, 12528, 16038, 16039, 16040, 16068] {
        assert_eq!(MountType::find_by_npc_id(strider), MountType::Strider);
    }
    assert_eq!(MountType::find_by_npc_id(12621), MountType::Wyvern);
    for wolf in [16030, 16037, 16041, 16042] {
        assert_eq!(MountType::find_by_npc_id(wolf), MountType::Wolf);
    }
    // The Great Wolf pet itself is not a mount, nor is nothing.
    assert_eq!(MountType::find_by_npc_id(16025), MountType::None);
    assert_eq!(MountType::find_by_npc_id(0), MountType::None);
    assert_eq!(
        [
            MountType::None,
            MountType::Strider,
            MountType::Wyvern,
            MountType::Wolf
        ]
        .map(MountType::ordinal),
        [0, 1, 2, 3]
    );
    // The data crate's `isMountable` is the same list.
    for npc_id in 0..17_000 {
        assert_eq!(
            MountType::find_by_npc_id(npc_id) != MountType::None,
            l2_data::pet::is_mountable(npc_id),
            "{npc_id}"
        );
    }
}

// ---- action 38 ---------------------------------------------------------------------------------

/// `RequestActionUse` 38 -> `Player.mountPlayer(summon)` -> `mount(pet)`: the pet goes away (the
/// owner gets `PetDelete`), `Ride` and `SetupGauge` (the food bar, colour 3) and the `UserInfo` with
/// the mount follow, and the collar is the mount's object id.
#[test]
fn action_38_mounts_the_strider() {
    let mut s = Scene::with_strider(80);
    let pet = s.summon_strider();
    s.me.take();

    s.action(38);
    let sent = s.me.take();
    assert!(s.world.is_mounted(ME));
    assert_eq!(s.world.mount_type_of(ME), MountType::Strider);
    assert_eq!(s.world.mount_npc_id_of(ME), 12526);
    // `setMountObjectID(pet.getControlObjectId())`.
    assert_eq!(s.world.mount_object_id_of(ME), CONTROL);
    // `pet.unSummon(this)`.
    assert_eq!(s.world.summon_of(ME), None);
    assert!(!of(&sent, "PET_DELETE").is_empty());
    let delete: PetDelete = decode(of(&sent, "PET_DELETE")[0]);
    assert_eq!(delete.pet_object_id, pet.0);

    // The player's own `Ride`: mounted, strider, the NPC id +1000000 on the wire.
    let ride: Ride = decode(of(&sent, "RIDE")[0]);
    assert_eq!(
        (ride.object_id, ride.mounted, ride.ride_type, ride.ride_npc_id),
        (ME.0, true, 1, 12526)
    );
    // The food bar: the pet's food (80), the level's maximum (100), over what it eats (2) a tick.
    let gauges: Vec<SetupGauge> = of(&sent, "SETUP_GAUGE").into_iter().map(|p| decode(p)).collect();
    assert!(!gauges.is_empty());
    assert!(
        gauges.iter().all(|g| g.object_id == ME.0
            && g.color == 3
            && g.time == 80 * 10_000 / 2
            && g.max_time == 100 * 10_000 / 2),
        "{gauges:?}"
    );
    // `UserInfo` with the mount type and `getMountNpcId() + 1000000`.
    let info: UserInfo = decode(of(&sent, "USER_INFO").last().unwrap());
    assert_eq!((info.mount_type, info.mount_npc_id), (1, 1_012_526));
    // The mount's collision (radius 19, height 26) replaces the character's.
    assert_eq!((info.collision_radius, info.collision_height), (19.0, 26.0));
    // `isMounted() ? 0 : getEnchantEffect()`.
    assert_eq!(info.enchant_effect, 0);
}

/// The ride speeds are the `speed_on_ride` of the pet level (`PlayerStat.getBaseMoveSpeed`); a mount
/// ten levels above the rider halves them (`getRunSpeed` / `getWalkSpeed`), a hungry mount halves
/// them again.
#[test]
fn the_ride_has_its_own_speeds_and_penalties() {
    let mut s = Scene::with_strider(80);
    let plain = s.view_run_speed();
    s.mounted_strider();
    // The strider (level 30) is ten levels above the rider (20): half of its 140 and 130.
    let view = s.world.stats(ME).unwrap();
    assert_eq!(view.base_move_speed(MoveType::Run), 140.0);
    assert_eq!(view.base_move_speed(MoveType::Walk), 130.0);
    assert_eq!(view.base_move_speed(MoveType::FastSwim), 75.0);
    assert_eq!(view.base_move_speed(MoveType::SlowSwim), 70.0);
    let half = view.run_speed();
    assert!(!view.is_hungry());
    assert!(half < plain, "{half} against the {plain} on foot");

    // Hungry (below 55 % of 100): half again, and everybody is told.
    s.me.take();
    s.world.set_current_feed(ME, 40);
    let sent = s.me.take();
    assert!(s.world.is_mount_hungry(ME));
    let hungry = s.view_run_speed();
    assert!((hungry * 2.0 - half).abs() < 1e-9, "{hungry} against {half}");
    assert!(!of(&sent, "USER_INFO").is_empty(), "hungry: UserInfo");
    // `setCurrentFeed` caps at the maximum and sends the bar.
    s.world.set_current_feed(ME, 5_000);
    assert_eq!(s.world.current_feed_of(ME), 100);
    assert!(!s.world.is_mount_hungry(ME));
}

/// A mount of the rider's own level (`mount(npcId, 0, false)`, the admin commands) is not halved.
#[test]
fn a_mount_of_the_riders_level_is_full_speed() {
    let mut s = Scene::new(vec![], None, false);
    let plain = s.view_run_speed();
    assert!(s.world.mount_npc(ME, 12526, ObjectId(0), false));
    // The pet data of the strider has no row for level 20: no ride speeds, the class's own.
    assert_eq!(s.world.stats(ME).unwrap().base_move_speed(MoveType::Run), 120.0);
    assert_eq!(s.view_run_speed(), plain);

    // The wyvern has a row for level 20.
    assert!(s.world.mount_npc(ME, 12621, ObjectId(0), false));
    assert_eq!(s.world.stats(ME).unwrap().base_move_speed(MoveType::Run), 260.0);
}

/// `mountPlayer` refuses with `ActionFailed` and the message of each check, in Java's order.
#[test]
fn mount_checks_refuse_with_java_s_messages() {
    // Hungry: 10 of 100 is below the 55 % hunger limit.
    let mut s = Scene::with_strider(10);
    s.summon_strider();
    s.me.take();
    s.action(38);
    let sent = s.me.take();
    assert!(!s.world.is_mounted(ME));
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::AHungryStriderCannotBeMountedOrDismounted]
    );
    assert!(names(&sent).contains(&"ACTION_FAIL".to_owned()));
    assert!(s.world.summon_of(ME).is_some(), "the strider stays");

    // Sitting.
    let mut s = Scene::with_strider(80);
    s.summon_strider();
    s.action(0);
    s.advance(1500);
    s.me.take();
    s.action(38);
    assert_eq!(
        message_ids(&s.me.take()),
        [SystemMessageId::AStriderCanBeRiddenOnlyWhenStanding]
    );

    // Holding a combat flag (9819). EnterWorld destroys a flag that the player logs in with
    // (Java "Remove combat flag before teleporting", M7-FORT-01), so it is added afterwards.
    let mut s = Scene::new(
        vec![item(CONTROL, STRIDER_COLLAR, 1, "INVENTORY")],
        Some(80),
        false,
    );
    s.world.add_item(ME, 9819, 1, false);
    s.summon_strider();
    s.me.take();
    s.action(38);
    let sent = s.me.take();
    assert!(!s.world.is_mounted(ME));
    assert!(names(&sent).contains(&"ACTION_FAIL".to_owned()));
    let flag = SystemMessage::text("You cannot mount a steed while holding a flag.").to_bytes();
    assert!(of(&sent, "SYSTEM_MESSAGE").into_iter().any(|p| *p == flag));

    // Dead rider.
    let mut s = Scene::with_strider(80);
    s.summon_strider();
    s.world.set_dead(ME, true);
    s.me.take();
    // A dead player's actions stop at the common checks of `RequestActionUse` (ActionFailed only).
    s.action(38);
    assert!(!s.world.is_mounted(ME));
}

/// Without a pet there is nothing to mount: action 38 does nothing (`mountPlayer(null)` returns
/// true, no packet).
#[test]
fn action_38_without_a_pet_does_nothing() {
    let mut s = Scene::new(vec![], None, false);
    s.action(38);
    assert!(s.me.take().is_empty());
    assert!(!s.world.is_mounted(ME));
}

// ---- dismounting -------------------------------------------------------------------------------

/// `dismount()`: `SetupGauge(3, 0, 0)`, `Ride` (not mounted, 1000000), the user info, and
/// `storePetFood` writes `UPDATE pets SET fed` of the saddle.
#[test]
fn dismount_stores_the_food_and_tells_everybody() {
    let mut s = Scene::with_strider(80);
    s.mounted_strider();
    // After 10 s the mount ate its 2.
    s.advance(10_000);
    assert_eq!(s.world.current_feed_of(ME), 78);
    s.me.take();
    s.persisted.summon_writes();

    s.action(38);
    let sent = s.me.take();
    assert!(!s.world.is_mounted(ME));
    assert_eq!(s.world.mount_object_id_of(ME), ObjectId(0));
    let gauge: SetupGauge = decode(of(&sent, "SETUP_GAUGE")[0]);
    assert_eq!(
        (gauge.object_id, gauge.color, gauge.time, gauge.max_time),
        (ME.0, 3, 0, 0)
    );
    let ride: Ride = decode(of(&sent, "RIDE")[0]);
    assert_eq!((ride.mounted, ride.ride_type, ride.ride_npc_id), (false, 0, 0));
    let info: UserInfo = decode(of(&sent, "USER_INFO").last().unwrap());
    assert_eq!((info.mount_type, info.mount_npc_id), (0, 0));
    assert_eq!(
        s.persisted.summon_writes(),
        [SummonWrite::PetFed {
            control_object_id: CONTROL,
            fed: 78
        }]
    );
    // The writes are once per ride: the collar is forgotten.
    s.world.store_pet_food(ME, 12526);
    assert!(s.persisted.summon_writes().is_empty());
}

/// A hungry mount cannot be dismounted (`isHungry()`): `A_HUNGRY_STRIDER_CANNOT_BE_MOUNTED_OR_DISMOUNTED`.
#[test]
fn a_hungry_strider_cannot_be_dismounted() {
    let mut s = Scene::with_strider(80);
    s.mounted_strider();
    s.world.set_current_feed(ME, 40);
    s.me.take();
    s.action(38);
    let sent = s.me.take();
    assert!(s.world.is_mounted(ME));
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::AHungryStriderCannotBeMountedOrDismounted]
    );
}

/// The user commands 61 and 62 are `mountPlayer(getSummon())` and `Dismount`.
#[test]
fn the_user_commands_mount_and_dismount() {
    let mut s = Scene::with_strider(80);
    s.summon_strider();
    s.input(ClientInput::UserCommand { id: 61 });
    assert!(s.world.is_mounted(ME));
    s.input(ClientInput::UserCommand { id: 62 });
    assert!(!s.world.is_mounted(ME));
    // A command 62 without a ride does nothing.
    s.me.take();
    s.input(ClientInput::UserCommand { id: 62 });
    assert!(s.me.take().is_empty());
}

// ---- the food ----------------------------------------------------------------------------------

/// `PetFeedTask`: every 10 s the mount eats `consume_meal_in_normal`; an empty bar ends the ride
/// with `YOU_ARE_OUT_OF_FEED_MOUNT_STATUS_CANCELED`.
#[test]
fn the_mount_eats_and_starves() {
    let mut s = Scene::with_strider(80);
    s.mounted_strider();
    s.advance(9_900);
    assert_eq!(s.world.current_feed_of(ME), 80, "not yet");
    s.advance(100);
    assert_eq!(s.world.current_feed_of(ME), 78);
    s.advance(10_000);
    assert_eq!(s.world.current_feed_of(ME), 76);

    // Starving: 2 left is not more than the 2 it eats.
    s.world.set_current_feed(ME, 2);
    s.me.take();
    s.advance(10_000);
    let sent = s.me.take();
    assert!(!s.world.is_mounted(ME));
    assert_eq!(s.world.current_feed_of(ME), 0);
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::YouAreOutOfFeedMountStatusCanceled]
    );
    // The task is gone with the ride.
    s.me.take();
    s.advance(30_000);
    assert!(s.me.take().is_empty());
}

/// A hungry mount eats the rider's food (`PetFood` with `isMounted()`): the item goes, the
/// skill animation plays, the bar rises by the skill's `feed`, and the rider is told.
#[test]
fn a_hungry_mount_eats_the_riders_food() {
    let mut s = Scene::new(
        vec![
            item(CONTROL, STRIDER_COLLAR, 1, "INVENTORY"),
            item(FOOD_STACK, FOOD, 3, "INVENTORY"),
        ],
        Some(80),
        false,
    );
    s.summon_strider();
    s.action(38);
    // It got hungry on the way (a hungry strider cannot be mounted, but it can be ridden hungry).
    s.world.set_current_feed(ME, 50);
    assert!(s.world.is_mount_hungry(ME), "50 of 100 is below 55 %");
    s.me.take();
    s.advance(10_000);
    let sent = s.me.take();
    // 50 - 2 = 48, still hungry: one food (+100, capped at 100).
    assert_eq!(s.world.current_feed_of(ME), 100);
    assert!(!s.world.is_mount_hungry(ME));
    let left: i64 = s
        .world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == FOOD)
        .map(|i| i.count)
        .sum();
    assert_eq!(left, 2);
    assert!(message_ids(&sent).contains(&SystemMessageId::YourPetWasHungrySoItAteS1));
    assert!(!of(&sent, "MAGIC_SKILL_USE").is_empty());
}

/// Food on a player who rides nothing: `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS`.
#[test]
fn pet_food_is_refused_without_a_mount() {
    let mut s = Scene::new(vec![item(FOOD_STACK, FOOD, 3, "INVENTORY")], None, false);
    s.input(ClientInput::UseItem {
        object_id: FOOD_STACK.0,
        ctrl_pressed: false,
    });
    let sent = s.me.take();
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms]
    );
}

// ---- what a ride takes away --------------------------------------------------------------------

/// `mount` disarms the weapon first (`disarmWeapons`, `disarmShield`), an equipped weapon cannot
/// be put on while mounted (`UseItem`), a mounted player has no stores and no toggles, and the
/// saddle cannot be destroyed.
#[test]
fn a_ride_disarms_and_blocks() {
    let mut weapon = item(SWORD_ID, SWORD, 1, "PAPERDOLL");
    // `Inventory.PAPERDOLL_RHAND`.
    weapon.loc_data = 5;
    let mut s = Scene::new(
        vec![item(CONTROL, STRIDER_COLLAR, 1, "INVENTORY"), weapon],
        Some(80),
        false,
    );
    s.summon_strider();
    s.me.take();
    s.action(38);
    // `sendInventoryUpdate` waits 100 ms.
    s.advance(300);
    let sent = s.me.take();
    assert!(s.world.is_mounted(ME));
    let sword = s
        .world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.object_id == SWORD_ID)
        .unwrap()
        .clone();
    assert_eq!(sword.loc, "INVENTORY", "the sword came off");
    assert!(message_ids(&sent).contains(&SystemMessageId::S1HasBeenDisarmed));
    assert!(!of(&sent, "INVENTORY_UPDATE").is_empty());

    // Putting it on again: "You do not meet the required condition to equip that item."
    s.me.take();
    s.input(ClientInput::UseItem {
        object_id: SWORD_ID.0,
        ctrl_pressed: false,
    });
    assert_eq!(
        message_ids(&s.me.take()),
        [SystemMessageId::YouDoNotMeetTheRequiredConditionToEquipThatItem]
    );

    // The saddle of the mount cannot be destroyed.
    s.me.take();
    s.input(ClientInput::DestroyItem {
        object_id: CONTROL.0,
        count: 1,
    });
    assert_eq!(
        message_ids(&s.me.take()),
        [SystemMessageId::AsYourPetIsCurrentlyOutItsSummoningItemCannotBeDestroyed]
    );
    assert!(
        s.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .any(|i| i.object_id == CONTROL)
    );
}

/// A mounted player cannot sit (`useSit` returns first), cannot duel (`canDuel`), cannot use a
/// summon item (`SummonItems`: `hasSummon() || isMounted()`), and a leaving player stores the
/// food of the ride.
#[test]
fn a_mounted_player_cannot_sit_or_duel_and_leaving_stores_the_food() {
    let mut s = Scene::with_strider(80);
    assert_eq!(s.world.can_duel(ME), Ok(()));
    s.mounted_strider();
    s.me.take();
    s.action(0);
    s.advance(1500);
    assert!(!s.world.is_sitting(ME));
    assert!(s.me.take().is_empty(), "`useSit` returns before it does anything");

    // `Player.canDuel()`: "currently riding a boat, steed or strider".
    assert_eq!(
        s.world.can_duel(ME),
        Err(SystemMessageId::C1CannotDuelBecauseC1IsCurrentlyRidingABoatSteedOrStrider)
    );

    // The saddle again: "You already have a pet."
    s.input(ClientInput::UseItem {
        object_id: CONTROL.0,
        ctrl_pressed: false,
    });
    assert_eq!(message_ids(&s.me.take()), [SystemMessageId::YouAlreadyHaveAPet]);

    s.persisted.summon_writes();
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Logout,
    });
    assert!(s.persisted.summon_writes().contains(&SummonWrite::PetFed {
        control_object_id: CONTROL,
        fed: 80
    }));
}

// ---- the wyvern --------------------------------------------------------------------------------

/// `setMount(wyvern)`: the rider flies (the fly speeds of `UserInfo`), gets Wyvern Breath in the
/// skill list; the dismount takes both away.
#[test]
fn a_wyvern_flies_and_has_wyvern_breath() {
    let mut s = Scene::new(vec![], None, false);
    assert!(s.world.mount_npc(ME, 12621, ObjectId(0), false));
    s.advance(500);
    let sent = s.me.take();
    assert!(s.world.is_flying(ME));
    assert_eq!(s.world.mount_type_of(ME), MountType::Wyvern);
    let ride: Ride = decode(of(&sent, "RIDE")[0]);
    assert_eq!((ride.mounted, ride.ride_type, ride.ride_npc_id), (true, 2, 12621));
    let info: UserInfo = decode(of(&sent, "USER_INFO").last().unwrap());
    assert_eq!(info.mount_type, 2);
    assert_ne!(info.fly_run_speed, 0);
    assert_eq!(info.fly_run_speed, info.run_speed);
    let list: SkillList = decode(of(&sent, "SKILL_LIST").last().unwrap());
    assert!(list.skills.iter().any(|e| e.id == WYVERN_BREATH_ID));
    // A skill with a negative effect is refused while mounted, except Wyvern Breath.
    assert!(
        !s.world
            .check_skill_condition(ME, ME, effects::SkillKey::new(3050, 1))
    );
    assert!(message_ids(&s.me.take()).contains(&SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms));
    assert!(
        s.world
            .check_skill_condition(ME, ME, effects::SkillKey::new(WYVERN_BREATH_ID, 1))
    );

    assert!(s.world.dismount(ME));
    s.advance(500);
    let sent = s.me.take();
    assert!(!s.world.is_flying(ME));
    let list: SkillList = decode(of(&sent, "SKILL_LIST").last().unwrap());
    assert!(!list.skills.iter().any(|e| e.id == WYVERN_BREATH_ID));
    let info: UserInfo = decode(of(&sent, "USER_INFO").last().unwrap());
    assert_eq!((info.mount_type, info.fly_run_speed), (0, 0));
}

/// The rider cannot dismount in the air (`z > 10000`) or from more than 300 above the ground
/// outside a no-landing zone; a GM can.
#[test]
fn dismounting_needs_a_place_to_land() {
    let mut s = Scene::new(vec![], None, false);
    s.world.mount_npc(ME, 12621, ObjectId(0), false);
    s.me.take();
    s.world.tele_to_location(ME, TeleportTarget::at(X, Y, 12_000));
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Appearing,
    });
    s.me.take();
    assert!(!s.world.dismount(ME));
    let sent = s.me.take();
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::YouAreNotAllowedToDismountInThisLocation]
    );
    assert!(names(&sent).contains(&"ACTION_FAIL".to_owned()));
    assert!(s.world.is_mounted(ME));

    let mut gm = Scene::new(vec![], None, true);
    gm.world.mount_npc(ME, 12621, ObjectId(0), false);
    gm.world.tele_to_location(ME, TeleportTarget::at(X, Y, 12_000));
    gm.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Appearing,
    });
    assert!(gm.world.dismount(ME));
}

struct ZoneRoot(std::path::PathBuf);

impl ZoneRoot {
    fn new(tag: &str, zone_xml: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-mount-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["zones", "mapregion"] {
            std::fs::create_dir_all(dir.join("data").join(sub)).unwrap();
        }
        std::fs::write(
            dir.join("data/zones/test.xml"),
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">{zone_xml}</list>\n"),
        )
        .unwrap();
        std::fs::write(
            dir.join("data/mapregion/talking_island_town.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n\
             </region>\n</list>\n",
        )
        .unwrap();
        Self(dir)
    }
}

impl Drop for ZoneRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn no_landing_zone() -> String {
    format!(
        "<zone id=\"1\" name=\"nl\" type=\"NoLandingZone\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
         <stat name=\"dismountDelay\" val=\"2\"/>\
         <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone>\n",
        X - 500,
        Y - 500,
        X + 500,
        Y + 500
    )
}

/// `NoLandingZone.onEnter`: a wyvern rider is told and dismounted after the zone's `dismountDelay`
/// (2 s here), unless it leaves first (`exitedNoLanding`); a GM gets the notice only.
#[test]
fn a_no_landing_zone_dismounts_a_wyvern_rider() {
    let root = ZoneRoot::new("landing", &no_landing_zone());
    let enter = |s: &mut Scene| {
        s.world.set_zone_data(
            Arc::new(Zones::load(&root.0)),
            Arc::new(MapRegions::load(&root.0)),
        );
        s.world.tele_to_location(ME, TeleportTarget::at(X + 4_000, Y, Z));
        s.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Appearing,
        });
        s.world.mount_npc(ME, 12621, ObjectId(0), false);
        s.world.tele_to_location(ME, TeleportTarget::at(X, Y, Z));
        s.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Appearing,
        });
    };

    let mut s = Scene::new(vec![], None, false);
    enter(&mut s);
    s.advance(100);
    let sent = s.me.take();
    assert!(message_ids(&sent).contains(
        &SystemMessageId::ThisAreaCannotBeEnteredWhileMountedAtopOfAWyvernYouWillBeDismountedFromYourWyvernIfYouDoNotLeave
    ));
    assert!(s.world.is_mounted(ME));
    // The wyvern may not be dismounted by hand inside the zone.
    s.me.take();
    s.action(38);
    assert!(message_ids(&s.me.take()).contains(&SystemMessageId::YouAreNotAllowedToDismountInThisLocation));
    s.advance(1_800);
    assert!(s.world.is_mounted(ME), "the delay is 2 s");
    s.advance(400);
    assert!(!s.world.is_mounted(ME), "dismounted by the zone's task");

    // Leaving in time cancels the task.
    let mut s = Scene::new(vec![], None, false);
    enter(&mut s);
    s.advance(100);
    s.world.tele_to_location(ME, TeleportTarget::at(X + 4_000, Y, Z));
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Appearing,
    });
    s.advance(5_000);
    assert!(s.world.is_mounted(ME));

    // A GM only hears the notice.
    let mut s = Scene::new(vec![], None, true);
    enter(&mut s);
    s.advance(5_000);
    assert!(s.world.is_mounted(ME));
    let notice =
        SystemMessage::text("You have entered a no-landing zone. Dismount restrictions are ignored for GMs.")
            .to_bytes();
    let sent = s.me.take();
    assert!(of(&sent, "SYSTEM_MESSAGE").into_iter().any(|p| *p == notice));
}

// ---- the rented pet ----------------------------------------------------------------------------

/// `startRentPet(seconds)` / `RentPetTask` / `stopRentPet`: the rented pet is dismounted when the
/// time is up; `/dismount` (62) ends it early.
#[test]
fn a_rented_pet_ends_with_its_time() {
    let mut s = Scene::new(vec![], None, false);
    s.world.mount_npc(ME, 12526, ObjectId(0), true);
    assert!(!s.world.is_rented_pet(ME));
    s.world.start_rent_pet(ME, 3);
    assert!(s.world.is_rented_pet(ME));
    s.advance(2_900);
    assert!(s.world.is_mounted(ME));
    s.advance(200);
    assert!(!s.world.is_mounted(ME));
    assert!(!s.world.is_rented_pet(ME));

    let mut s = Scene::new(vec![], None, false);
    s.world.mount_npc(ME, 12621, ObjectId(0), false);
    s.world.start_rent_pet(ME, 600);
    s.input(ClientInput::UserCommand { id: 62 });
    assert!(!s.world.is_mounted(ME));
    assert!(!s.world.is_rented_pet(ME));
}

// ---- the Ride effect ---------------------------------------------------------------------------

/// The `Ride` skill effect: `mount(npcId, 0, false)`.
#[test]
fn the_ride_effect_mounts_the_npc() {
    let mut s = Scene::new(vec![], None, false);
    s.world.apply_effects(ME, ME, effects::SkillKey::new(3060, 1));
    assert_eq!(s.world.mount_type_of(ME), MountType::Strider);
    assert_eq!(s.world.mount_npc_id_of(ME), 12526);
    assert_eq!(s.world.mount_object_id_of(ME), ObjectId(0));
}

// ---- the free mounts ---------------------------------------------------------------------------

/// `FreeMountsConfig` (`rewardSkills`): Twilight Dragon (49991) and Wind Dragon (49992) are given
/// when their switch is on and the datapack has the skill (the test data lacks Wind Dragon).
#[test]
fn the_free_mount_skills_follow_their_switches() {
    let s = Scene::build(vec![], None, false, (false, false));
    assert!(s.world.player(ME).unwrap().core.skills.is_empty());
    let s = Scene::build(vec![], None, false, (true, true));
    assert_eq!(s.world.player(ME).unwrap().core.skills, [(49991, 1)]);
    let s = Scene::build(vec![], None, false, (false, true));
    assert!(s.world.player(ME).unwrap().core.skills.is_empty());
}

// ---- the packet shape --------------------------------------------------------------------------

/// A player that is not mounted still sends 1000000 as the NPC id of `Ride`.
#[test]
fn ride_of_an_unmounted_player_carries_the_offset() {
    let bytes = Ride {
        object_id: 1,
        mounted: false,
        ride_type: 0,
        ride_npc_id: 0,
        x: 0,
        y: 0,
        z: 0,
    }
    .to_bytes();
    assert_eq!(&bytes[13..17], &1_000_000_i32.to_le_bytes());
}
