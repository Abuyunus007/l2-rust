#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Pets (M6-PET-01): the collar and `SummonPet`, the pet's numbers from its level row, the feed
//! task and the pet food, the pet inventory and its packets, the pickup, exp and levels, death and
//! resurrection, unsummoning and the rows Java writes (`pets`, `character_pet_skills_save`), the
//! name, the baby pet's heals. Expected values follow the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, ItemTemplates, KarmaLoss,
    NpcLoadOptions, NpcTemplates, PetDataTable, PetSkillData, PlayerTemplates, SkillTemplates, SkillTrees,
    SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    ExChangeNpcState, PetDelete, PetInfo, PetInventoryUpdate, PetItemList, PetStatusUpdate,
    PrivateStoreManageListSell, SummonInfo, SystemMessage,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::effects::SkillKey;
use l2_world::stats::{BaseStatBonus, StatData};
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

/// `(object id, loc, count)` of the upserted items and the object ids of the removed ones.
type ItemWrite = (Vec<(i32, String, i64)>, Vec<i32>);

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

    /// The `(upserted item ids, removed item ids)` of the item writes since the last call.
    fn item_writes(&self) -> Vec<ItemWrite> {
        let mut commands = self.0.lock().unwrap();
        let writes = commands
            .iter()
            .filter_map(|c| match c {
                PersistCommand::ItemChanges { upserts, removed, .. } => Some((
                    upserts
                        .iter()
                        .map(|i| (i.object_id.0, i.loc.clone(), i.count))
                        .collect(),
                    removed.iter().map(|o| o.0).collect(),
                )),
                _ => None,
            })
            .collect();
        commands.retain(|c| !matches!(c, PersistCommand::ItemChanges { .. }));
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

/// 2046 is the shape of the datapack's Wolf's Necklace (the collar's skill), 2048 of Wolves' Food
/// (`feed` 100), 4717 of a baby pet's Heal Trick (level 0 in the pet data = by the pet's level).
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
<skill id="4717" levels="10" name="Heal Trick">
	<operateType>A1</operateType>
	<targetType>OWNER_PET</targetType>
	<castRange>600</castRange>
	<effectPoint>100</effectPoint>
	<hitTime>500</hitTime>
	<isMagic>1</isMagic>
	<mpConsume>5</mpConsume>
	<power>40</power>
	<effects><effect name="Heal"><param power="40"/></effect></effects>
</skill>
<skill id="9301" levels="1" name="Pet Buff"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel><isMagic>1</isMagic>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9302" levels="1" name="Resurrect"><operateType>A1</operateType><targetType>PC_BODY</targetType>
<castRange>600</castRange><effectPoint>100</effectPoint><isMagic>1</isMagic>
<effects><effect name="Resurrection"><power>100</power></effect></effects></skill>
</list>"##;

const COLLAR: i32 = 6648;
const FOOD: i32 = 7582;
const ORE: i32 = 9100;
const SWORD: i32 = 9101;
const HEAVY: i32 = 9102;
const PET_ARMOR: i32 = 9103;

const ITEMS: &str = r##"<list>
<item id="6648" type="EtcItem" name="Baby Buffalo Panpipe">
	<set name="default_action" val="SKILL_MAINTAIN" /><set name="etcitem_type" val="PET_COLLAR" />
	<set name="weight" val="20" /><set name="handler" val="SummonItems" />
	<skills><skill id="2046" level="1" /></skills>
</item>
<item id="7582" type="EtcItem" name="Baby Spice">
	<set name="is_stackable" val="true" /><set name="for_npc" val="true" /><set name="weight" val="1" />
	<set name="handler" val="PetFood" />
	<skills><skill id="2048" level="1" /></skills>
</item>
<item id="9100" type="EtcItem" name="Test Ore">
	<set name="is_stackable" val="true" /><set name="weight" val="2" />
</item>
<item id="9101" type="EtcItem" name="Test Stick"><set name="weight" val="3" /></item>
<item id="9102" type="EtcItem" name="Test Anvil"><set name="weight" val="70000" /></item>
<item id="9103" type="Armor" name="Test Pet Coat">
	<set name="weight" val="10" /><set name="bodypart" val="chest" /><set name="armor_type" val="NONE" />
	<set name="for_npc" val="true" />
	<stats><stat type="pDef">20</stat></stats>
	<conditions><target npcId="12780" /></conditions>
</item>
</list>"##;

/// 12780 the datapack's Baby Buffalo (a `BabyPet`) at level 3; 9001 a monster.
const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="12780" level="3" type="BabyPet" name="Baby Buffalo">
    <parameters>
      <skill name="Heal1" id="4717" level="1" />
    </parameters>
    <race>BEAST</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="25">
      <vitals hp="460.96247" hpRegen="3.5" mp="259.2" mpRegen="1.5" />
      <attack physical="64.83203" magical="44.27187" random="10" critical="4" accuracy="4.75" attackSpeed="253" type="FIST" range="40" distance="80" width="120" />
      <defence physical="101.83533" magical="74.51868" />
      <speed><walk ground="30" /><run ground="120" /></speed>
    </stats>
    <shots soul="2" spirit="2" />
    <corpseTime>86400</corpseTime>
    <collision><radius normal="12"/><height normal="15"/></collision>
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

/// One level of the pet data: the exp it starts at and its HP.
fn pet_level(level: i32, exp: i64, hp: i32) -> String {
    format!(
        r#"<stat level="{level}"><set name="max_meal" val="20"/><set name="exp" val="{exp}"/>
        <set name="get_exp_type" val="70"/><set name="consume_meal_in_battle" val="3"/>
        <set name="consume_meal_in_normal" val="2"/><set name="org_pattack" val="10"/>
        <set name="org_pdefend" val="20"/><set name="org_mattack" val="15"/><set name="org_mdefend" val="25"/>
        <set name="org_hp" val="{hp}"/><set name="org_mp" val="80"/><set name="org_hp_regen" val="6"/>
        <set name="org_mp_regen" val="3"/><set name="soulshot_count" val="2"/>
        <set name="spiritshot_count" val="1"/></stat>"#
    )
}

/// Levels 1 to 6; level 6 is the cap (`getExpForLevel(maxLevel)`), reached by nobody.
fn pet_data() -> Arc<PetDataTable> {
    let levels: String = [
        (1, 0, 100),
        (2, 100, 120),
        (3, 300, 140),
        (4, 600, 160),
        (5, 1000, 180),
        (6, 2000, 200),
    ]
    .into_iter()
    .map(|(l, e, hp)| pet_level(l, e, hp))
    .collect();
    Arc::new(
        PetDataTable::from_xml(&format!(
            r#"<pets><pet id="12780" itemId="6648">
            <set name="food" val="7582"/><set name="hungry_limit" val="55"/><set name="load" val="54510"/>
            <skills><skill minLevel="1" skillId="4717" skillLevel="0"/></skills>
            <stats>{levels}</stats></pet></pets>"#
        ))
        .unwrap(),
    )
}

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let template = self.0.items.get(item_id)?;
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

/// Levels 1..=30 of `experience.xml`.
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
const P2: ObjectId = ObjectId(0x2000_0002);
/// The collar's object id.
const CONTROL: ObjectId = ObjectId(0x2100_0001);
const ORE_STACK: ObjectId = ObjectId(0x2100_0002);
const STICK: ObjectId = ObjectId(0x2100_0003);

fn item(object_id: ObjectId, item_id: i32, count: i64, loc: &str) -> ItemData {
    ItemData {
        object_id,
        item_id,
        count,
        loc: loc.into(),
        ..Default::default()
    }
}

fn collar() -> ItemData {
    item(CONTROL, COLLAR, 1, "INVENTORY")
}

fn player_data(id: ObjectId, x: i32, items: Vec<ItemData>, profile: PlayerProfile) -> PlayerData {
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
        level: 20,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 5,
        access_level: 0,
        x,
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

struct Scene {
    world: World,
    me: Log,
    p2: Log,
    persisted: Persisted,
}

impl Scene {
    fn new(items: Vec<ItemData>) -> Self {
        Self::with_profile(items, PlayerProfile::default())
    }

    fn with_profile(items: Vec<ItemData>, profile: PlayerProfile) -> Self {
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(persisted.clone()));
        world.set_static_data(Box::new(Data(stat_data())));
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
        let skills = stat_data().skills;
        world.set_pet_skill_data(Arc::new(
            PetSkillData::from_xml("<list></list>", &|_, _| true).unwrap(),
        ));
        let _ = skills;
        world.set_pet_data(pet_data());
        world.set_pet_settings(PetSettings {
            player_spawn_protection: 600,
            ..PetSettings::default()
        });
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(ME, X, items, profile)),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let p2 = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(2),
            player: Box::new(player_data(P2, X + 300, vec![], PlayerProfile::default())),
            sink: Box::new(Sink(p2.clone())),
            ip: None,
        });
        let mut scene = Self {
            world,
            me,
            p2,
            persisted,
        };
        scene.advance(1000);
        scene.me.take();
        scene.p2.take();
        scene.persisted.summon_writes();
        scene.persisted.item_writes();
        scene
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

    fn pet_input(&mut self, input: PetInput) {
        self.input(ClientInput::Pet(input));
    }

    fn action(&mut self, action_id: i32) {
        self.input(ClientInput::ActionUse {
            action_id,
            ctrl_pressed: false,
            shift_pressed: false,
        });
    }

    /// The collar double clicked: `UseItem` -> `SummonItems` -> the item skill 2046 -> `SummonPet`.
    fn use_collar(&mut self) -> ObjectId {
        self.input(ClientInput::UseItem {
            object_id: CONTROL.0,
            ctrl_pressed: false,
        });
        self.advance(1000);
        self.world.summon_of(ME).expect("a pet")
    }

    fn pet(&self) -> ObjectId {
        self.world.summon_of(ME).expect("a pet")
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

fn count_of(world: &World, owner: ObjectId, item_id: i32, pet: bool) -> i64 {
    world
        .player(owner)
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == item_id && (i.loc == "PET" || i.loc == "PET_EQUIP") == pet)
        .map(|i| i.count)
        .sum()
}

// ---- summoning ---------------------------------------------------------------------------------

/// `SummonItems` -> the collar's skill -> `SummonPet.onStart` -> `Pet.spawnPet`: the owner gets
/// `PetInfo` (type 2, the summon animation) and an (empty) `PetItemList`, everybody else
/// `SummonInfo`. A new pet has the level of its template (3), the exp and HP of that level's row,
/// a full stomach, no name, and the owner's name as title. `storeMe` inserts the row before the
/// spawn.
#[test]
fn the_collar_summons_a_new_pet() {
    let mut s = Scene::new(vec![collar()]);
    s.input(ClientInput::UseItem {
        object_id: CONTROL.0,
        ctrl_pressed: false,
    });
    s.advance(1000);
    let pet = s.pet();
    assert!(s.world.is_pet(pet) && !s.world.is_servitor(pet));
    assert!(s.world.is_baby_pet(pet));
    assert_eq!(s.world.summon_owner(pet), Some(ME));
    assert_eq!(s.world.pet_control_object_id(pet), Some(CONTROL));
    assert_eq!(s.world.object(pet).unwrap().kind, ObjectKind::Summon);

    let mine = s.me.take();
    let infos: Vec<PetInfo> = of(&mine, "PET_INFO").into_iter().map(|p| decode(p)).collect();
    assert!(!infos.is_empty(), "{:?}", names(&mine));
    let info = &infos[0];
    assert_eq!(info.summon_type, 2);
    assert_eq!(info.object_id, pet.0);
    assert_eq!(info.spawn_state, 2, "the summon animation");
    assert_eq!(info.level, 3);
    assert_eq!(info.name, "");
    assert_eq!(info.title, format!("P{}", ME.0));
    assert_eq!((info.cur_fed, info.max_fed), (20, 20));
    assert_eq!((info.max_hp, info.cur_hp), (140, 140), "org_hp of level 3");
    assert_eq!(
        (info.exp, info.exp_for_this_level, info.exp_for_next_level),
        (300, 300, 600)
    );
    assert_eq!((info.soul_shots_per_hit, info.spirit_shots_per_hit), (2, 1));
    assert!(!info.mountable);
    assert!(info.has_owner);
    let list: PetItemList = decode(of(&mine, "PET_ITEM_LIST")[0]);
    assert!(list.items.is_empty());

    let theirs = s.p2.take();
    let seen: Vec<SummonInfo> = of(&theirs, "NPC_INFO").into_iter().map(|p| decode(p)).collect();
    assert_eq!(seen[0].object_id, pet.0);
    assert_eq!(seen[0].name, "", "the pet has no name yet");

    // The pet appeared next to the owner, `spawnMe(x + 50, y + 100, z)`, and follows it.
    let (x, y, _) = s.world.position_of(pet).unwrap();
    assert!((x - X).abs() < 120 && (y - Y).abs() < 120, "{x} {y}");
    assert_eq!(s.world.intention_of(pet), Intention::Follow);
    // `item.setEnchantLevel(pet.getLevel())`.
    let collar = s
        .world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.object_id == CONTROL)
        .unwrap()
        .clone();
    assert_eq!(collar.enchant_level, 3);

    // `storeMe` ran before the pet was spawned: an INSERT, with `_restoreSummon` still true.
    let writes = s.persisted.summon_writes();
    match &writes[0] {
        SummonWrite::StorePet { insert, pet } => {
            assert!(insert);
            assert_eq!(pet.control_object_id, CONTROL);
            assert_eq!(pet.owner, ME);
            assert_eq!(
                (pet.name.clone(), pet.level, pet.exp, pet.fed),
                (None, 3, 300, 20)
            );
            assert_eq!((pet.cur_hp, pet.cur_mp), (140, 80));
            assert!(pet.restore, "NOTE(java-quirk): the first row says restore");
        }
        other => panic!("{other:?}"),
    }
    // ... and `CharSummonTable._pets` has the owner now: no second summon.
    assert!(
        matches!(writes.last(), Some(SummonWrite::PetEffects { .. })),
        "{writes:?}"
    );
}

/// `Pet.getPAtk/getMaxHp/...`: the numbers of the level row run through the calculators of an NPC.
#[test]
fn the_pet_numbers_come_from_the_level_row() {
    let mut s = Scene::new(vec![collar()]);
    let pet = s.use_collar();
    let stats = s.world.stats(pet).unwrap();
    assert_eq!(stats.level(), 3);
    assert_eq!(stats.max_hp(), 140);
    assert_eq!(stats.max_mp(), 80);
    // The row's value, times the calculators of an NPC: FuncPAtkMod (STR bonus and level mod
    // ((3 + 89) / 100)), FuncPDefMod, FuncMAtkMod (INT bonus), FuncMDefMod (MEN bonus).
    let level_mod = 0.92;
    assert_eq!(stats.level_mod(), level_mod);
    assert_eq!(stats.p_atk(None), 10.0 * stats.bonus(BaseStat::Str) * level_mod);
    assert_eq!(stats.p_def(None), 20.0 * level_mod);
    assert_eq!(
        stats.m_def(None, None),
        25.0 * stats.bonus(BaseStat::Men) * level_mod
    );
    // `PetData.getAvailableLevel`: the skill of level 0 at level 3 is level 1.
    assert_eq!(s.world.pet_available_skill_level(pet, 4717), 1);
}

/// `SummonItems`: a player who has a pet is told so and nothing else happens; `Player.setPet` holds
/// one summon at a time (`ConditionPlayerCanSummon`).
#[test]
fn a_second_summon_is_refused() {
    let mut s = Scene::new(vec![collar()]);
    let pet = s.use_collar();
    s.me.take();
    s.input(ClientInput::UseItem {
        object_id: CONTROL.0,
        ctrl_pressed: false,
    });
    s.advance(1000);
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::YouAlreadyHaveAPet]
    );
    assert_eq!(s.world.summon_of(ME), Some(pet));
    // The collar of a summoned pet cannot be destroyed (`RequestDestroyItem`).
    s.input(ClientInput::DestroyItem {
        object_id: CONTROL.0,
        count: 1,
    });
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::AsYourPetIsCurrentlyOutItsSummoningItemCannotBeDestroyed]
    );
}

// ---- food --------------------------------------------------------------------------------------

/// `FeedTask` every 10 s: the food bar goes down by `consume_meal_in_normal` (2) and the owner
/// sees `PetStatusUpdate`; at 10 of 20 the pet is hungry (`hungry_limit` 55 %) and eats what it
/// carries: "Your pet was hungry, so it ate $s1.", `PetFood` (skill 2048 feeds 100, the food bar
/// stops at the maximum).
#[test]
fn the_feed_task_eats_the_pets_food_when_it_is_hungry() {
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, FOOD, 3, "INVENTORY")]);
    let pet = s.use_collar();
    // The owner gives the pet its food.
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 3,
    });
    assert_eq!(count_of(&s.world, ME, FOOD, true), 3);
    s.me.take();

    s.advance(10_000);
    assert_eq!(s.world.pet_current_fed(pet), 18);
    let packets = s.me.take();
    let status: PetStatusUpdate = decode(of(&packets, "PET_STATUS_UPDATE").last().unwrap());
    assert_eq!((status.cur_fed, status.max_fed), (18, 20));
    assert!(!s.world.pet_is_hungry(pet));

    // 16, 14, 12, then 10: hungry, and it eats.
    s.advance(40_000);
    let packets = s.me.take();
    let ids = message_ids(&packets);
    assert!(
        ids.contains(&SystemMessageId::YourPetWasHungrySoItAteS1),
        "{ids:?}"
    );
    assert_eq!(
        s.world.pet_current_fed(pet),
        20,
        "10 + 100, capped at the maximum"
    );
    assert_eq!(count_of(&s.world, ME, FOOD, true), 2);
    // The food is not enough to say "still hungry".
    assert!(!ids.contains(&SystemMessageId::YourPetAteALittleButIsStillHungry));
    assert!(!of(&packets, "MAGIC_SKILL_USE").is_empty());
    let update: PetInventoryUpdate = decode(of(&packets, "PET_INVENTORY_UPDATE")[0]);
    assert_eq!(update.updates.len(), 1);
}

/// `RequestPetUseItem` with the pet food: it is eaten at once; a hungry pet that stays hungry says so.
#[test]
fn the_pet_food_item_feeds_the_pet() {
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, FOOD, 2, "INVENTORY")]);
    let pet = s.use_collar();
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 2,
    });
    let food = s.world.pet_inventory_summary(ME)[0].0;
    s.me.take();
    s.pet_input(PetInput::UseItem { object_id: food.0 });
    assert_eq!(s.world.pet_current_fed(pet), 20);
    assert_eq!(count_of(&s.world, ME, FOOD, true), 1);
    let packets = s.me.take();
    assert!(
        !of(&packets, "MAGIC_SKILL_USE").is_empty(),
        "{:?}",
        names(&packets)
    );
    assert!(!of(&packets, "PET_INVENTORY_UPDATE").is_empty());
    // A thing the pet cannot use: not for NPCs.
    let mut s = Scene::new(vec![collar(), item(STICK, SWORD, 1, "INVENTORY")]);
    s.use_collar();
    s.pet_input(PetInput::GiveItemToPet {
        object_id: STICK.0,
        amount: 1,
    });
    s.me.take();
    s.pet_input(PetInput::UseItem { object_id: STICK.0 });
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::ThisPetCannotUseThisItem]
    );
}

/// A pet that comes back with an empty stomach (`setCurrentFed(0)`) tells its owner
/// (`ExChangeNpcState` 0x64), is slow, and the feed task makes it leave or ignore orders:
/// `Rnd.get(100) < 30` it leaves ("Starving, grumpy and fed up, your pet has left.", the Owl
/// Monk's pk reduction is for the Owl Monk), else "Your pet is unresponsive ...". Orders are
/// refused with the message `RequestActionUse.validateSummon` sends (a Java copy and paste slip).
#[test]
fn a_starving_pet_is_slow_ignores_orders_and_leaves() {
    let row = SavedPet {
        control_object_id: CONTROL,
        name: Some("Buffy".into()),
        level: 3,
        cur_hp: 140,
        cur_mp: 80,
        exp: 300,
        sp: 0,
        fed: 0,
        restore: false,
    };
    let profile = PlayerProfile {
        pet_rows: vec![row],
        ..PlayerProfile::default()
    };
    let mut s = Scene::with_profile(vec![collar()], profile);
    let pet = s.use_collar();
    let packets = s.me.take();
    let state: ExChangeNpcState = decode(of(&packets, "EX_CHANGE_NPC_STATE")[0]);
    assert_eq!((state.object_id, state.state), (pet.0, 0x64));
    assert!(s.world.pet_is_uncontrollable(pet));
    // The restored row: its name, level, exp and HP.
    let info: PetInfo = decode(of(&packets, "PET_INFO")[0]);
    assert_eq!(
        (info.name.as_str(), info.level, info.exp, info.cur_hp),
        ("Buffy", 3, 300, 140)
    );

    // A starving pet runs at half the speed, twice (`getMovementSpeedMultiplier`).
    let stats = s.world.stats(pet).unwrap();
    let base = stats.base_move_speed(l2_world::stats::MoveType::Run);
    assert_eq!(
        stats.run_speed(),
        stats.calc(l2_data::Stat::MoveSpeed, base) * 0.5
    );

    // The orders.
    s.action(16);
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::OnlyAClanLeaderThatIsANoblesseCanViewTheSiegeWarStatusWindowDuringASiegeWar]
    );

    // The feed task: each run is "unresponsive" until one run is the 30 %.
    let mut unresponsive = 0;
    let mut left = false;
    for _ in 0..40 {
        s.advance(10_000);
        let packets = s.me.take();
        let ids = message_ids(&packets);
        if ids.contains(&SystemMessageId::StarvingGrumpyAndFedUpYourPetHasLeft) {
            left = true;
            assert!(!of(&packets, "PET_DELETE").is_empty());
            break;
        }
        assert!(ids.contains(&SystemMessageId::YourPetServitorIsUnresponsiveAndWillNotObeyAnyOrders));
        unresponsive += 1;
    }
    assert!(left, "30 % each run: gone after {unresponsive} runs");
    assert_eq!(s.world.summon_of(ME), None);
    // `unSummon` stores the pet: an UPDATE of its row with the name and no restore.
    let writes = s.persisted.summon_writes();
    let stored = writes
        .iter()
        .rev()
        .find_map(|w| match w {
            SummonWrite::StorePet { insert, pet } => Some((*insert, pet.clone())),
            _ => None,
        })
        .unwrap();
    assert!(!stored.0);
    assert_eq!(
        (stored.1.name.as_deref(), stored.1.fed, stored.1.restore),
        (Some("Buffy"), 0, false)
    );
}

// ---- the pet inventory -------------------------------------------------------------------------

/// `Player.transferItem` / `Pet.transferItem`: part of a stack moves as a new stack, a whole item
/// moves as itself, a stack the pet has grows, everything comes back by `RequestGetItemFromPet`.
/// The rows change like `Item.updateDatabase` (the owner's id stays, `loc` becomes `PET`).
#[test]
fn items_move_between_the_owner_and_the_pet() {
    let mut s = Scene::new(vec![
        collar(),
        item(ORE_STACK, ORE, 10, "INVENTORY"),
        item(STICK, SWORD, 1, "INVENTORY"),
    ]);
    s.use_collar();
    s.me.take();
    s.persisted.item_writes();

    // Part of the stack: a new stack for the pet.
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 4,
    });
    assert_eq!(count_of(&s.world, ME, ORE, false), 6);
    assert_eq!(count_of(&s.world, ME, ORE, true), 4);
    let packets = s.me.take();
    let update: PetInventoryUpdate = decode(of(&packets, "PET_INVENTORY_UPDATE")[0]);
    assert_eq!(update.updates.len(), 1);
    assert_eq!(
        update.updates[0].change,
        l2_protocol_game::server::InventoryUpdate::ADDED
    );
    assert_eq!(update.updates[0].item.count, 4);
    let load = of(&packets, "STATUS_UPDATE");
    assert!(!load.is_empty(), "the owner's CUR_LOAD");
    let writes = s.persisted.item_writes();
    assert_eq!(writes.len(), 2, "{writes:?}");
    assert_eq!(writes[0].0, vec![(ORE_STACK.0, "INVENTORY".to_owned(), 6)]);
    assert_eq!(writes[1].0.len(), 1);
    assert_eq!((writes[1].0[0].1.as_str(), writes[1].0[0].2), ("PET", 4));

    // A whole item moves as itself.
    s.pet_input(PetInput::GiveItemToPet {
        object_id: STICK.0,
        amount: 1,
    });
    assert!(
        s.world
            .pet_inventory_summary(ME)
            .iter()
            .any(|i| i.0 == STICK && i.3 == "PET")
    );
    assert_eq!(count_of(&s.world, ME, SWORD, false), 0);
    assert_eq!(
        s.persisted.item_writes(),
        vec![(vec![(STICK.0, "PET".to_owned(), 1)], vec![])]
    );

    // More ore: the stack the pet has grows.
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 2,
    });
    assert_eq!(count_of(&s.world, ME, ORE, true), 6);
    assert_eq!(s.world.pet_inventory_summary(ME).len(), 2);
    s.me.take();

    // Back: the pet's whole stack goes to the owner's stack, and its row is gone.
    let pet_stack = s
        .world
        .pet_inventory_summary(ME)
        .into_iter()
        .find(|i| i.1 == ORE)
        .unwrap()
        .0;
    s.persisted.item_writes();
    s.pet_input(PetInput::GetItemFromPet {
        object_id: pet_stack.0,
        amount: 6,
    });
    assert_eq!(count_of(&s.world, ME, ORE, false), 10);
    assert_eq!(count_of(&s.world, ME, ORE, true), 0);
    let packets = s.me.take();
    let update: PetInventoryUpdate = decode(of(&packets, "PET_INVENTORY_UPDATE")[0]);
    assert_eq!(
        update.updates[0].change,
        l2_protocol_game::server::InventoryUpdate::REMOVED
    );
    let writes = s.persisted.item_writes();
    assert!(writes.iter().any(|w| w.1.contains(&pet_stack.0)), "{writes:?}");

    // The stick comes back as itself.
    s.pet_input(PetInput::GetItemFromPet {
        object_id: STICK.0,
        amount: 1,
    });
    assert_eq!(count_of(&s.world, ME, SWORD, false), 1);
    assert!(s.world.pet_inventory_summary(ME).is_empty());
}

/// `RequestGiveItemToPet`: more than the owner has, a dead pet, no room (`MaximumSlotsForPet`) and
/// too heavy items are refused with Java's messages.
#[test]
fn giving_items_to_the_pet_has_limits() {
    let stick2 = ObjectId(0x2100_0004);
    let stick3 = ObjectId(0x2100_0005);
    let anvil = ObjectId(0x2100_0006);
    let mut s = Scene::new(vec![
        collar(),
        item(ORE_STACK, ORE, 10, "INVENTORY"),
        item(STICK, SWORD, 1, "INVENTORY"),
        item(stick2, SWORD, 1, "INVENTORY"),
        item(stick3, SWORD, 1, "INVENTORY"),
        item(anvil, HEAVY, 1, "INVENTORY"),
    ]);
    s.world.set_pet_settings(PetSettings {
        inventory_maximum_pet: 2,
        ..PetSettings::default()
    });
    let pet = s.use_collar();
    s.me.take();

    // More than the owner has: Java punishes the player; nothing moves.
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 11,
    });
    assert_eq!(count_of(&s.world, ME, ORE, true), 0);

    // Too heavy (`validateWeight`).
    s.pet_input(PetInput::GiveItemToPet {
        object_id: anvil.0,
        amount: 1,
    });
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::YourPetCannotCarryAnyMoreItems]
    );

    // Two slots.
    s.pet_input(PetInput::GiveItemToPet {
        object_id: STICK.0,
        amount: 1,
    });
    s.pet_input(PetInput::GiveItemToPet {
        object_id: stick2.0,
        amount: 1,
    });
    s.me.take();
    s.pet_input(PetInput::GiveItemToPet {
        object_id: stick3.0,
        amount: 1,
    });
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::YourPetCannotCarryAnyMoreItems]
    );
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 1,
    });
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::YourPetCannotCarryAnyMoreItems]
    );
    assert_eq!(s.world.pet_inventory_summary(ME).len(), 2);

    // The collar of the summoned pet is not an item to give (`checkItemManipulation` ->
    // `validateItemManipulation`), and a dead pet takes nothing.
    s.pet_input(PetInput::GiveItemToPet {
        object_id: CONTROL.0,
        amount: 1,
    });
    assert_eq!(s.world.pet_inventory_summary(ME).len(), 2);
    s.world.reduce_current_hp(pet, 100_000.0, Some(P2));
    s.me.take();
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 1,
    });
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::YourPetIsDeadAndAnyAttemptYouMakeToGiveItSomethingGoesUnrecognized]
    );
}

/// `RequestPetGetItem`: the pet walks to the item and `Pet.doPickupItem` puts it into its own
/// inventory: `GetItem` for everybody, "Your pet picked up $s2 $s1(s).", `PetInventoryUpdate`, the
/// row. The owner's inventory is not touched.
#[test]
fn the_pet_picks_up_items() {
    let mut s = Scene::new(vec![collar()]);
    let pet = s.use_collar();
    let (px, py, pz) = s.world.position_of(pet).unwrap();
    s.world.load_ground_items(vec![GroundItemRow {
        object_id: ObjectId(0x2200_0001),
        item_id: ORE,
        count: 5,
        enchant_level: 0,
        x: px + 60,
        y: py,
        z: pz,
        drop_time: -1,
        equipable: false,
    }]);
    s.advance(100);
    s.me.take();
    s.p2.take();
    s.persisted.item_writes();

    s.pet_input(PetInput::GetItem {
        object_id: 0x2200_0001,
    });
    s.advance(3000);
    assert!(s.world.ground_item(ObjectId(0x2200_0001)).is_none(), "picked up");
    assert_eq!(count_of(&s.world, ME, ORE, true), 5);
    assert_eq!(count_of(&s.world, ME, ORE, false), 0);
    let packets = s.me.take();
    let ids = message_ids(&packets);
    assert!(ids.contains(&SystemMessageId::YourPetPickedUpS2S1S), "{ids:?}");
    assert!(!of(&packets, "GET_ITEM").is_empty(), "{:?}", names(&packets));
    assert!(!of(&packets, "PET_INVENTORY_UPDATE").is_empty());
    let writes = s.persisted.item_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!((writes[0].0[0].1.as_str(), writes[0].0[0].2), ("PET", 5));
    // The pet follows its owner again.
    assert_eq!(s.world.intention_of(pet), Intention::Follow);
}

/// A pet without food orders: `RequestPetGetItem` of an item that is not there is "action failed";
/// of a starving pet "You are not allowed to enter the party room" (a Java slip).
#[test]
fn the_pet_get_item_request_checks_the_pet() {
    let mut s = Scene::new(vec![collar()]);
    // No pet.
    s.pet_input(PetInput::GetItem { object_id: 12345 });
    assert_eq!(names(&s.me.take()), vec!["ACTION_FAIL"]);
    s.use_collar();
    s.me.take();
    s.pet_input(PetInput::GetItem { object_id: 12345 });
    assert_eq!(names(&s.me.take()), vec!["ACTION_FAIL"]);
}

// ---- exp and levels ----------------------------------------------------------------------------

/// `PlayerStat.addExpAndSp`: a pet in range takes `get_exp_type` (70) percent from the owner; the
/// pet gets the other part of the exp (`Pet.addExpAndSp` -> `PetStat.addExpAndSp`): "Your pet
/// gained $s1 experience points.", and its level follows its exp table: `PlayableStat.addExp` ->
/// `PetStat.addLevel`: `StatusUpdate` and the level up effect for everybody, full HP and MP.
#[test]
fn the_pet_shares_the_owners_exp_and_levels_up() {
    let mut s = Scene::new(vec![collar()]);
    let pet = s.use_collar();
    s.world.reduce_current_hp(pet, 20.0, Some(P2));
    s.me.take();
    s.p2.take();

    s.world.add_exp_and_sp(ME, 1000.0, 10.0, false);
    // The owner keeps 70 %, the pet gets the 30 % (the SP is not used by pets).
    assert_eq!(s.world.player(ME).unwrap().data.exp, 700);
    assert_eq!(s.world.summon(pet).unwrap().exp, 600, "300 + 300");
    // 600 is where level 4 starts.
    assert_eq!(s.world.stats(pet).unwrap().level(), 4);
    let packets = s.me.take();
    assert!(
        message_ids(&packets).contains(&SystemMessageId::YourPetGainedS1ExperiencePoints),
        "{:?}",
        message_ids(&packets)
    );
    let info: PetInfo = decode(of(&packets, "PET_INFO").last().unwrap());
    assert_eq!(
        (info.level, info.max_hp, info.cur_hp),
        (4, 160, 160),
        "full HP of the new level"
    );
    assert_eq!(
        (info.exp, info.exp_for_this_level, info.exp_for_next_level),
        (600, 600, 1000)
    );
    assert!(!of(&packets, "SOCIAL_ACTION").is_empty(), "the level up effect");
    assert!(!of(&packets, "STATUS_UPDATE").is_empty());
    // The viewers hear about the level too.
    assert!(!of(&s.p2.take(), "SOCIAL_ACTION").is_empty());
    // The collar shows the level.
    let enchant = s
        .world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.object_id == CONTROL)
        .unwrap()
        .enchant_level;
    assert_eq!(enchant, 4);
}

/// A dead pet gets nothing, but the owner still gives up the pet's share (`PlayerStat.addExpAndSp`
/// multiplies by the ratio whether or not the pet is alive).
#[test]
fn a_dead_pet_takes_no_exp_but_the_owner_still_gives_up_its_share() {
    let mut s = Scene::new(vec![collar()]);
    let pet = s.use_collar();
    s.world.reduce_current_hp(pet, 100_000.0, Some(P2));
    let pet_exp = s.world.summon(pet).unwrap().exp;
    s.world.add_exp_and_sp(ME, 1000.0, 0.0, false);
    assert_eq!(s.world.player(ME).unwrap().data.exp, 700);
    assert_eq!(s.world.summon(pet).unwrap().exp, pet_exp);
}

// ---- death -------------------------------------------------------------------------------------

/// `Pet.doDie`: the death penalty (`(-0.07 * level) + 6.5` percent of the level's exp span), "The
/// pet has been killed ...", the corpse decays after its template's `corpseTime`. No stored buffs
/// (`Summon.doDie(killer)` is skipped). The dead pet is resurrected through its owner: the question
/// (`ConfirmDlg`), the answer, `Pet.doRevive(power)` brings the exp back and the feed task runs.
#[test]
fn a_dead_pet_loses_exp_and_is_resurrected_through_its_owner() {
    let mut s = Scene::new(vec![collar()]);
    let pet = s.use_collar();
    s.persisted.summon_writes();
    s.me.take();

    s.world.reduce_current_hp(pet, 100_000.0, Some(P2));
    assert!(s.world.is_dead(pet));
    assert!(s.world.has_decay_task(pet));
    // 300 - round((600 - 300) * 6.29 / 100) = 281: below level 3.
    assert_eq!(s.world.summon(pet).unwrap().exp, 281);
    assert_eq!(s.world.stats(pet).unwrap().level(), 2);
    assert_eq!(s.world.pet_state(pet).unwrap().exp_before_death, 300);
    let ids = message_ids(&s.me.take());
    assert!(
        ids.contains(
            &SystemMessageId::ThePetHasBeenKilledIfYouDonTResurrectItWithin24HoursThePetSBodyWillDisappearAlongWithAllThePetSItems
        ),
        "{ids:?}"
    );
    assert!(ids.contains(&SystemMessageId::YourPetReceivedS2DamageByC1));
    // Dead pets do not eat.
    s.advance(10_000);
    assert_eq!(s.world.pet_current_fed(pet), 20);
    // The summon-pet effect keeps no buffs of the dead.
    assert!(
        s.persisted
            .summon_writes()
            .iter()
            .all(|w| !matches!(w, SummonWrite::PetEffects { .. }))
    );

    // The resurrection question is for the pet, not the owner (who lives).
    s.world.revive_request(ME, P2, true, 100);
    let asked = s.me.take();
    assert!(!of(&asked, "CONFIRM_DLG").is_empty(), "{:?}", names(&asked));
    assert!(s.world.is_reviving_pet(ME));
    s.world.revive_answer(ME, 1);
    assert!(!s.world.is_dead(pet));
    assert!(!s.world.has_decay_task(pet));
    // The lost exp comes back by the power: 281 + round((300 - 281) * 100 / 100).
    assert_eq!(s.world.summon(pet).unwrap().exp, 300);
    assert_eq!(s.world.stats(pet).unwrap().level(), 3);
    assert!(!s.world.is_reviving_pet(ME));
    assert!(!of(&s.me.take(), "REVIVE").is_empty());
    // The feed task runs again.
    s.advance(10_000);
    assert_eq!(s.world.pet_current_fed(pet), 18);
}

/// Nobody asks for the resurrection of a pet that lives (`pet && hasPet() && _summon.isDead()`).
#[test]
fn a_living_pet_is_not_asked_to_come_back() {
    let mut s = Scene::new(vec![collar()]);
    s.use_collar();
    s.me.take();
    s.world.revive_request(ME, P2, true, 100);
    assert!(s.me.take().is_empty());
    assert!(!s.world.is_revive_requested(ME));
}

/// A dead pet decays after its corpse time: `Pet.deleteMe`: the items go back to the owner, the
/// collar is destroyed ("$s1 has disappeared.") and the `pets` row goes.
#[test]
fn the_decay_of_a_dead_pet_destroys_its_collar_and_returns_its_items() {
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, ORE, 10, "INVENTORY")]);
    let pet = s.use_collar();
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 4,
    });
    s.world.reduce_current_hp(pet, 100_000.0, Some(P2));
    s.me.take();
    s.persisted.summon_writes();
    s.advance(86_400_000 / 50 * 50 / 1000 * 1000 + 1000);
    assert!(s.world.summon_of(ME).is_none());
    assert!(s.world.object(pet).is_none());
    let packets = s.me.take();
    assert!(!of(&packets, "PET_DELETE").is_empty(), "{:?}", names(&packets));
    assert!(message_ids(&packets).contains(&SystemMessageId::S1HasDisappeared));
    // The ore is back with the owner, the collar is gone.
    assert_eq!(count_of(&s.world, ME, ORE, false), 10);
    assert_eq!(count_of(&s.world, ME, ORE, true), 0);
    assert!(
        s.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .all(|i| i.item_id != COLLAR)
    );
    let writes = s.persisted.summon_writes();
    assert!(
        writes.contains(&SummonWrite::DeletePet {
            control_object_id: CONTROL
        }),
        "{writes:?}"
    );
}

// ---- unsummoning and the rows ------------------------------------------------------------------

/// Action 19: a pet that is hungry (under 40 % of its food) cannot be sent back ("You may not
/// restore a hungry pet."); a fed one goes: `PetDelete`, `storeMe` (an UPDATE, no restore) and
/// `storeEffect(true)`; it carries items, so the owner is told and the collar cannot be dropped.
#[test]
fn action_19_unsummons_a_fed_pet_and_writes_the_rows() {
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, ORE, 10, "INVENTORY")]);
    s.action(19);
    assert_eq!(message_ids(&s.me.take()), vec![SystemMessageId::YouDoNotHaveAPet]);

    let pet = s.use_collar();
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 4,
    });
    s.me.take();
    s.persisted.summon_writes();

    // 7 runs of the feed task: 6 of 20 is under 40 %.
    s.advance(70_000);
    assert_eq!(s.world.pet_current_fed(pet), 6);
    s.me.take();
    s.action(19);
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::YouMayNotRestoreAHungryPet]
    );
    assert_eq!(s.world.summon_of(ME), Some(pet));
    s.world.pet_set_current_fed(pet, 20);
    s.world.apply_effects(ME, pet, SkillKey::new(9301, 1));
    s.advance(100);

    s.persisted.summon_writes();
    s.action(19);
    let packets = s.me.take();
    let delete: PetDelete = decode(of(&packets, "PET_DELETE")[0]);
    assert_eq!((delete.pet_type, delete.pet_object_id), (2, pet.0));
    assert!(message_ids(&packets).contains(
        &SystemMessageId::ThereAreItemsInYourPetInventoryRenderingYouUnableToSellTradeDropPetSummoningItemsPleaseEmptyYourPetInventory
    ));
    assert_eq!(s.world.summon_of(ME), None);
    assert!(s.world.object(pet).is_none());
    let writes = s.persisted.summon_writes();
    match &writes[0] {
        SummonWrite::StorePet { insert, pet } => {
            assert!(!insert, "the row exists");
            assert_eq!((pet.level, pet.fed, pet.restore), (3, 20, false));
        }
        other => panic!("{other:?}"),
    }
    match &writes[1] {
        SummonWrite::PetEffects {
            control_object_id,
            effects,
        } => {
            assert_eq!(*control_object_id, CONTROL);
            assert_eq!(effects.len(), 1);
            assert_eq!(effects[0].skill_id, 9301);
        }
        other => panic!("{other:?}"),
    }
    // The items stay with the pet (rows of the owner), the collar cannot be dropped.
    assert_eq!(count_of(&s.world, ME, ORE, true), 4);
    assert!(s.world.have_pet_inv_items(ME));
    s.input(ClientInput::DropItem {
        object_id: CONTROL.0,
        count: 1,
        x: X,
        y: Y,
        z: Z,
    });
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::ThisItemCannotBeDiscarded]
    );

    // The pet comes back from its row, with its items, its buff and its level, and not as a
    // new pet: no INSERT.
    s.persisted.summon_writes();
    let again = s.use_collar();
    assert_eq!(s.world.stats(again).unwrap().level(), 3);
    assert_eq!(s.world.pet_current_fed(again), 20);
    assert!(s.world.effect_icons(again).iter().any(|i| i.skill_id == 9301));
    let list: PetItemList = decode(of(&s.me.take(), "PET_ITEM_LIST").last().unwrap());
    assert_eq!(list.items.len(), 1);
    let writes = s.persisted.summon_writes();
    assert!(
        writes
            .iter()
            .all(|w| !matches!(w, SummonWrite::StorePet { insert: true, .. })),
        "{writes:?}"
    );
}

/// `Player.deleteMe`: the pet is stored for a restore (`setRestoreSummon(true)`): the row says
/// `restore = true`.
#[test]
fn the_owner_leaving_stores_the_pet_for_a_restore() {
    let mut s = Scene::new(vec![collar()]);
    let pet = s.use_collar();
    s.persisted.summon_writes();
    s.input(ClientInput::Logout);
    assert!(s.world.object(pet).is_none());
    let writes = s.persisted.summon_writes();
    match &writes[0] {
        SummonWrite::StorePet { insert, pet } => {
            assert!(!insert);
            assert!(pet.restore);
        }
        other => panic!("{other:?}"),
    }
    assert!(!of(&s.p2.take(), "DELETE_OBJECT").is_empty());
}

/// `CharSummonTable.restorePet` at the first action of a player whose pet was stored for a restore
/// (`RestorePetOnReconnect`, spawn protection): the level, exp, food, name and HP come from the
/// `pets` row, nothing is inserted; until then a collar cannot be used (the owner is in
/// `CharSummonTable._pets`).
#[test]
fn a_stored_pet_comes_back_at_the_first_action() {
    let row = SavedPet {
        control_object_id: CONTROL,
        name: Some("Buffy".into()),
        level: 4,
        cur_hp: 90,
        cur_mp: 40,
        exp: 700,
        sp: 5,
        fed: 11,
        restore: true,
    };
    let profile = PlayerProfile {
        pet_rows: vec![row],
        ..PlayerProfile::default()
    };
    let mut s = Scene::with_profile(vec![collar()], profile);
    assert_eq!(s.world.summon_of(ME), None);
    // Using the collar is "multiple pets" for now, though it first counts as an action ...
    s.input(ClientInput::UseItem {
        object_id: CONTROL.0,
        ctrl_pressed: false,
    });
    s.advance(1000);
    let pet = s.pet();
    // ... which brought the stored pet back (`UseItem` -> `onActionRequest`).
    let stats = s.world.stats(pet).unwrap();
    assert_eq!(stats.level(), 4);
    assert_eq!(s.world.summon(pet).unwrap().exp, 700);
    assert_eq!(s.world.summon(pet).unwrap().sp, 5);
    assert_eq!(s.world.summon(pet).unwrap().name, "Buffy");
    assert_eq!(s.world.pet_current_fed(pet), 11);
    assert_eq!(s.world.npc(pet).unwrap().core.cur_hp, 90.0);
    let writes = s.persisted.summon_writes();
    assert!(
        writes
            .iter()
            .all(|w| !matches!(w, SummonWrite::StorePet { insert: true, .. })),
        "{writes:?}"
    );
}

/// A pet stored with no HP comes back dead (`rs.getDouble("curHp") < 1`).
#[test]
fn a_pet_stored_dead_comes_back_dead() {
    let row = SavedPet {
        control_object_id: CONTROL,
        name: None,
        level: 3,
        cur_hp: 0,
        cur_mp: 0,
        exp: 300,
        sp: 0,
        fed: 20,
        restore: true,
    };
    let profile = PlayerProfile {
        pet_rows: vec![row],
        ..PlayerProfile::default()
    };
    let mut s = Scene::with_profile(vec![collar()], profile);
    s.world.pet_on_action_request(ME);
    let pet = s.pet();
    assert!(s.world.is_dead(pet));
}

// ---- the name ----------------------------------------------------------------------------------

/// `RequestChangePetName`: a pet gets a name once; the checks and their messages are Java's
/// (taken name, 1 to 8 characters, no space, alphanumeric); a name is written at once and the
/// collar's custom type 2 says "named".
#[test]
fn a_pet_can_be_named_once() {
    let mut s = Scene::new(vec![collar()]);
    s.world.load_pet_names([("Dup".to_owned(), COLLAR)]);
    s.pet_input(PetInput::ChangeName { name: "Buffy".into() });
    assert!(s.me.take().is_empty(), "without a summon nothing happens");
    let pet = s.use_collar();
    s.me.take();
    for (name, expected) in [
        ("DUP", SystemMessageId::ThisIsAlreadyInUseByAnotherPet),
        ("", SystemMessageId::YourPetSNameCanBeUpTo8CharactersInLength),
        (
            "Abcdefghi",
            SystemMessageId::YourPetSNameCanBeUpTo8CharactersInLength,
        ),
        ("A b", SystemMessageId::ThereIsASpaceInTheName),
        ("Bu!", SystemMessageId::AnInvalidCharacterIsIncludedInThePetSName),
    ] {
        s.pet_input(PetInput::ChangeName { name: name.into() });
        assert_eq!(message_ids(&s.me.take()), vec![expected], "{name:?}");
    }
    s.persisted.summon_writes();
    s.pet_input(PetInput::ChangeName { name: "Buffy".into() });
    assert_eq!(s.world.summon(pet).unwrap().name, "Buffy");
    let packets = s.me.take();
    let info: PetInfo = decode(of(&packets, "PET_INFO").last().unwrap());
    assert_eq!(info.name, "Buffy");
    let writes = s.persisted.summon_writes();
    match &writes[0] {
        SummonWrite::StorePet { insert, pet } => {
            assert!(!insert);
            assert_eq!(pet.name.as_deref(), Some("Buffy"));
        }
        other => panic!("{other:?}"),
    }
    let collar = s
        .world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.object_id == CONTROL)
        .unwrap()
        .clone();
    assert_eq!(collar.custom_type2, 1);
    // Once.
    s.pet_input(PetInput::ChangeName { name: "Other".into() });
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::YouCannotSetTheNameOfThePet]
    );
}

// ---- clicks and orders -------------------------------------------------------------------------

/// `PetClick`: the first click targets, the second on the own pet opens its status window.
#[test]
fn clicking_the_own_pet_shows_its_status() {
    let mut s = Scene::new(vec![collar()]);
    let pet = s.use_collar();
    s.me.take();
    s.input(ClientInput::Action {
        object_id: pet.0,
        action_id: 0,
    });
    assert_eq!(s.world.target_of(ME), Some(pet));
    s.me.take();
    s.input(ClientInput::Action {
        object_id: pet.0,
        action_id: 0,
    });
    let packets = s.me.take();
    assert!(
        !of(&packets, "PET_STATUS_SHOW").is_empty(),
        "{:?}",
        names(&packets)
    );
}

// ---- the baby pet ------------------------------------------------------------------------------

/// `BabyPet.CastTask` every 2 s (the first after 3 s): an owner under 30 % HP (the group of the
/// baby pets counts as "improved", a slip of Java's names) is healed by the major heal with a 75 %
/// chance; the owner hears "Your pet uses $s1.". In the auxiliary mode (action 1084) it does
/// nothing and refuses skill orders.
#[test]
fn a_baby_pet_heals_its_owner() {
    let mut s = Scene::new(vec![collar()]);
    let pet = s.use_collar();
    assert!(s.world.baby_is_in_support_mode(pet));
    let max_hp = f64::from(s.world.stats(ME).unwrap().max_hp());
    let low = (max_hp * 0.1).floor();
    s.world.set_current_hp(ME, low, false);
    s.me.take();
    let mut used = false;
    for _ in 0..20 {
        s.advance(2000);
        let packets = s.me.take();
        if message_ids(&packets).contains(&SystemMessageId::YourPetUsesS1) {
            used = true;
            break;
        }
    }
    assert!(used, "75 % each run");
    s.advance(2000);
    assert!(s.world.player(ME).unwrap().core.cur_hp > low, "the heal landed");

    // The auxiliary mode: the heals stop.
    s.action(1084);
    assert!(!s.world.baby_is_in_support_mode(pet));
    s.world.set_current_hp(ME, low, false);
    s.me.take();
    s.advance(30_000);
    assert!(!message_ids(&s.me.take()).contains(&SystemMessageId::YourPetUsesS1));
    s.action(1041);
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::APetOnAuxiliaryModeCannotUseSkills]
    );
    // NOTE(java-quirk): `canControl` refuses before `switchMode()` runs, so the button that
    // switched to the auxiliary mode cannot switch back.
    s.action(1084);
    assert_eq!(
        message_ids(&s.me.take()),
        vec![SystemMessageId::APetOnAuxiliaryModeCannotUseSkills]
    );
    assert!(!s.world.baby_is_in_support_mode(pet));
}

// ---- items on the pet --------------------------------------------------------------------------

/// `RequestPetUseItem` with a pet armor: `Inventory.equipItem` puts it on the chest, its
/// functions join the pet's (P. Def), "Your pet put on $s1.", `PetItemList`, `PetInfo` shows the
/// armor; the armor stays on when the pet is sent away and comes back with it.
#[test]
fn the_pet_wears_its_armor() {
    let coat = ObjectId(0x2100_0007);
    let mut s = Scene::new(vec![collar(), item(coat, PET_ARMOR, 1, "INVENTORY")]);
    let pet = s.use_collar();
    s.pet_input(PetInput::GiveItemToPet {
        object_id: coat.0,
        amount: 1,
    });
    let before = s.world.stats(pet).unwrap().p_def(None);
    s.me.take();
    s.persisted.item_writes();
    s.pet_input(PetInput::UseItem { object_id: coat.0 });
    let packets = s.me.take();
    assert!(
        message_ids(&packets).contains(&SystemMessageId::YourPetPutOnS1),
        "{:?}",
        message_ids(&packets)
    );
    let list: PetItemList = decode(of(&packets, "PET_ITEM_LIST")[0]);
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].equipped, 1);
    let after = s.world.stats(pet).unwrap().p_def(None);
    assert!(after > before, "{before} {after}");
    let info: PetInfo = decode(of(&packets, "PET_INFO").last().unwrap());
    assert_eq!(info.armor, PET_ARMOR);
    assert_eq!(
        s.world.pet_inventory_summary(ME),
        vec![(coat, PET_ARMOR, 1, "PET_EQUIP".to_owned())]
    );
    assert_eq!(s.persisted.item_writes().last().unwrap().0[0].1, "PET_EQUIP");

    // Away and back: the coat is still on.
    s.action(19);
    let again = s.use_collar();
    assert_eq!(s.world.stats(again).unwrap().p_def(None), after);

    // Off.
    s.me.take();
    s.pet_input(PetInput::UseItem { object_id: coat.0 });
    assert!(message_ids(&s.me.take()).contains(&SystemMessageId::YourPetTookOffS1));
    assert_eq!(s.world.stats(again).unwrap().p_def(None), before);
}

// ---- a collar the world has no row for ---------------------------------------------------------

impl Persisted {
    /// The `(owner, collar)` of the pet row requests since the last call.
    fn pet_row_requests(&self) -> Vec<(ObjectId, ObjectId)> {
        let mut commands = self.0.lock().unwrap();
        let requests = commands
            .iter()
            .filter_map(|c| match c {
                PersistCommand::PetRowRequest {
                    owner,
                    control_object_id,
                } => Some((*owner, *control_object_id)),
                _ => None,
            })
            .collect();
        commands.retain(|c| !matches!(c, PersistCommand::PetRowRequest { .. }));
        requests
    }
}

/// A collar that came to the character after the login (a trade, a pickup) may have a `pets` row
/// the world never read. Using it asks the persistence task for the row (no insert, no pet yet,
/// one request however often the player clicks); the pet comes with the answer: a restore when
/// the row exists (its level and exp are not overwritten, D-39), a new pet when the database has
/// none, nothing when the read failed.
#[test]
fn an_unknown_collar_asks_for_its_row_before_the_pet_is_made() {
    let answer = |s: &mut Scene, control: ObjectId, result| {
        s.world.handle(WorldCommand::PetRowLoaded {
            owner: ME,
            control_object_id: control,
            result,
        });
    };
    let row = |control| SavedPet {
        control_object_id: control,
        name: Some("Old".into()),
        level: 4,
        cur_hp: 90,
        cur_mp: 40,
        exp: 700,
        sp: 0,
        fed: 11,
        restore: false,
    };

    // The row exists: a restore, not an insert.
    let mut s = Scene::new(vec![]);
    let control = s.world.add_item(ME, COLLAR, 1, false).unwrap();
    s.persisted.summon_writes();
    for _ in 0..2 {
        s.input(ClientInput::UseItem {
            object_id: control.0,
            ctrl_pressed: false,
        });
        s.advance(1000);
    }
    assert_eq!(s.world.summon_of(ME), None);
    assert_eq!(s.persisted.pet_row_requests(), vec![(ME, control)], "one request");
    assert!(s.persisted.summon_writes().is_empty(), "nothing is inserted");
    answer(
        &mut s,
        control,
        Ok(PetRowData {
            row: Some(row(control)),
            effects: vec![],
        }),
    );
    let pet = s.pet();
    assert_eq!(s.world.stats(pet).unwrap().level(), 4);
    assert_eq!(s.world.summon(pet).unwrap().exp, 700);
    assert_eq!(s.world.summon(pet).unwrap().name, "Old");
    assert!(
        s.persisted
            .summon_writes()
            .iter()
            .all(|w| !matches!(w, SummonWrite::StorePet { insert: true, .. }))
    );
    // It is known now: no second request.
    s.action(19);
    s.advance(100);
    s.input(ClientInput::UseItem {
        object_id: control.0,
        ctrl_pressed: false,
    });
    s.advance(1000);
    assert!(s.world.summon_of(ME).is_some());
    assert!(s.persisted.pet_row_requests().is_empty());

    // The database has no row: a new pet, inserted.
    let mut s = Scene::new(vec![]);
    let control = s.world.add_item(ME, COLLAR, 1, false).unwrap();
    s.input(ClientInput::UseItem {
        object_id: control.0,
        ctrl_pressed: false,
    });
    s.advance(1000);
    s.persisted.summon_writes();
    answer(
        &mut s,
        control,
        Ok(PetRowData {
            row: None,
            effects: vec![],
        }),
    );
    let pet = s.pet();
    assert_eq!(s.world.stats(pet).unwrap().level(), 3);
    assert!(
        s.persisted
            .summon_writes()
            .iter()
            .any(|w| matches!(w, SummonWrite::StorePet { insert: true, .. }))
    );

    // The read failed: no pet, and the player can try again.
    let mut s = Scene::new(vec![]);
    let control = s.world.add_item(ME, COLLAR, 1, false).unwrap();
    s.input(ClientInput::UseItem {
        object_id: control.0,
        ctrl_pressed: false,
    });
    s.advance(1000);
    s.persisted.summon_writes();
    answer(&mut s, control, Err(()));
    assert_eq!(s.world.summon_of(ME), None);
    assert!(s.persisted.summon_writes().is_empty());
    s.input(ClientInput::UseItem {
        object_id: control.0,
        ctrl_pressed: false,
    });
    s.advance(1000);
    assert_eq!(s.persisted.pet_row_requests().len(), 2);
    // An answer nobody asked for is ignored.
    answer(
        &mut s,
        ObjectId(1),
        Ok(PetRowData {
            row: None,
            effects: vec![],
        }),
    );
    assert_eq!(s.world.summon_of(ME), None);
}

/// `ItemManager.destroyItem` of a collar on the ground (the auto destroy) deletes the `pets` row.
#[test]
fn a_collar_destroyed_on_the_ground_takes_its_row_with_it() {
    let mut s = Scene::new(vec![collar()]);
    s.world.load_ground_items(vec![GroundItemRow {
        object_id: ObjectId(0x2200_0009),
        item_id: COLLAR,
        count: 1,
        enchant_level: 0,
        x: X + 10,
        y: Y,
        z: Z,
        drop_time: 1,
        equipable: false,
    }]);
    s.persisted.summon_writes();
    // Long enough for the default 10 minutes of `AutoDestroyDroppedItemAfter`.
    s.advance(700_000);
    assert!(s.world.ground_item(ObjectId(0x2200_0009)).is_none());
    assert_eq!(
        s.persisted.summon_writes(),
        vec![SummonWrite::DeletePet {
            control_object_id: ObjectId(0x2200_0009)
        }]
    );
}

// ---- M6-FU-09: the collar rule and the decay of an offline owner's pet -----------------------------

/// The collar and its pet's ore, after the pet was sent away with the ore still in its inventory.
fn scene_with_a_pet_that_kept_its_items() -> Scene {
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, ORE, 10, "INVENTORY")]);
    let pet = s.use_collar();
    s.pet_input(PetInput::GiveItemToPet {
        object_id: ORE_STACK.0,
        amount: 4,
    });
    s.world.pet_set_current_fed(pet, 20);
    s.action(19);
    assert_eq!(s.world.summon_of(ME), None);
    s.me.take();
    s
}

/// The object ids of the inventory items the private sell store window offers
/// (`getAvailableItems(sellList)`, which asks `Item.isAvailable`).
fn store_window_items(s: &mut Scene) -> Vec<i32> {
    s.input(ClientInput::Store(StoreInput::ActionUse { action_id: 10 }));
    let packets = s.me.take();
    let raw = of(&packets, "PRIVATE_STORE_SELL_MANAGE_LIST");
    assert!(!raw.is_empty(), "{:?} {:?}", names(&packets), message_ids(&packets));
    let window: PrivateStoreManageListSell = decode(raw[0]);
    window.item_list.iter().map(|c| c.item.object_id).collect()
}

/// Java `Item.isAvailable`: the control item of the summoned pet is not available, and a pet collar
/// is not while `player.havePetInvItems()`. Trade, shops, warehouses and the post item list all
/// ask it, the sell store window stands for them here.
#[test]
fn a_collar_whose_pet_is_out_or_kept_items_is_not_available() {
    // No pet out, no pet items: the collar is offered.
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, ORE, 10, "INVENTORY")]);
    assert_eq!(store_window_items(&mut s), vec![CONTROL.0, ORE_STACK.0]);

    // The pet is out: "Not Control item of currently summoned pet."
    // (A store next to the pet itself is refused by `ShopMinRangeFromNpc`, so the range is off.)
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, ORE, 10, "INVENTORY")]);
    s.world.set_settings(WorldSettings {
        store: StoreSettings {
            shop_min_range_from_npc: 0,
            shop_min_range_from_player: 0,
            ..StoreSettings::default()
        },
        ..WorldSettings::default()
    });
    s.use_collar();
    s.me.take();
    assert_eq!(store_window_items(&mut s), vec![ORE_STACK.0]);

    // The pet went away with items (`havePetInvItems`): no collar, the rest stays.
    let mut s = scene_with_a_pet_that_kept_its_items();
    assert!(s.world.have_pet_inv_items(ME));
    assert_eq!(store_window_items(&mut s), vec![ORE_STACK.0]);
}

/// `Player.restorePetInventoryItems` at the login: one `PET` row of the owner is enough.
#[test]
fn pet_items_in_the_rows_at_the_login_block_the_collar() {
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, ORE, 4, "PET")]);
    assert!(s.world.have_pet_inv_items(ME));
    assert_eq!(store_window_items(&mut s), Vec::<i32>::new());

    let s = Scene::new(vec![collar()]);
    assert!(!s.world.have_pet_inv_items(ME));
}

impl Persisted {
    /// The offline pet reads the world asked for, taken out of the log.
    fn offline_pet_requests(&self) -> Vec<(ObjectId, ObjectId)> {
        let mut commands = self.0.lock().unwrap();
        let found = commands
            .iter()
            .filter_map(|c| match c {
                PersistCommand::OfflinePetRowsRequest {
                    owner,
                    control_object_id,
                } => Some((*owner, *control_object_id)),
                _ => None,
            })
            .collect();
        commands.retain(|c| !matches!(c, PersistCommand::OfflinePetRowsRequest { .. }));
        found
    }

    /// The ops of the `ItemRows` commands since the last call.
    fn item_row_ops(&self) -> Vec<ItemRowOp> {
        let mut commands = self.0.lock().unwrap();
        let ops = commands
            .iter()
            .flat_map(|c| match c {
                PersistCommand::ItemRows { tracked, ops } => {
                    assert_eq!(*tracked, None, "the owner is offline");
                    ops.clone()
                }
                _ => Vec::new(),
            })
            .collect();
        commands.retain(|c| !matches!(c, PersistCommand::ItemRows { .. }));
        ops
    }
}

const PET_ORE: ObjectId = ObjectId(0x2100_0010);
const PET_STICK: ObjectId = ObjectId(0x2100_0011);

/// A dead pet whose owner logged out, until its corpse decays and the world asks for the rows.
fn decayed_while_the_owner_is_offline() -> Scene {
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, ORE, 10, "INVENTORY")]);
    let pet = s.use_collar();
    s.world.reduce_current_hp(pet, 100_000.0, Some(P2));
    s.input(ClientInput::Logout);
    assert!(s.world.player(ME).is_none());
    // The dead pet stayed.
    assert!(s.world.object(pet).is_some());
    s.persisted.summon_writes();
    s.persisted.item_row_ops();
    s.advance(86_400_000 / 50 * 50 / 1000 * 1000 + 1000);
    assert!(s.world.object(pet).is_none());
    s
}

/// `Pet.deleteMe(owner)` of an owner who is offline: Java's retained `Player` object gets the
/// items and loses the collar, with the rows written. The world reads the rows first (D-15); a
/// stack goes to the owner's stack, a single item moves (a worn one comes off), the collar's row
/// and the `pets` row are deleted.
#[test]
fn a_corpse_that_decays_while_the_owner_is_offline_returns_the_items_in_the_rows() {
    let mut s = decayed_while_the_owner_is_offline();
    assert_eq!(s.persisted.offline_pet_requests(), vec![(ME, CONTROL)]);
    // Nothing is written before the answer.
    assert!(s.persisted.item_row_ops().is_empty());
    assert!(s.persisted.summon_writes().is_empty());

    s.world.handle(WorldCommand::OfflinePetRowsLoaded {
        owner: ME,
        control_object_id: CONTROL,
        result: Ok(OfflinePetRows {
            pet_items: vec![
                item(PET_ORE, ORE, 4, "PET"),
                // Not stackable, worn by the pet.
                ItemData {
                    loc_data: 6,
                    ..item(PET_STICK, SWORD, 1, "PET_EQUIP")
                },
            ],
            inventory: vec![collar(), item(ORE_STACK, ORE, 6, "INVENTORY")],
        }),
    });
    assert_eq!(
        s.persisted.item_row_ops(),
        vec![
            // The ore joins the owner's stack; the pet's row is gone.
            ItemRowOp::Update {
                owner_id: ME.0,
                item: item(ORE_STACK, ORE, 10, "INVENTORY"),
            },
            ItemRowOp::Delete { object_id: PET_ORE },
            // The single item is an inventory row now.
            ItemRowOp::Update {
                owner_id: ME.0,
                item: item(PET_STICK, SWORD, 1, "INVENTORY"),
            },
            // `destroyControlItem`.
            ItemRowOp::Delete { object_id: CONTROL },
        ]
    );
    assert_eq!(
        s.persisted.summon_writes(),
        vec![SummonWrite::DeletePet {
            control_object_id: CONTROL
        }]
    );
}

/// `ItemContainer.transferItem` of a stackable item the owner has no stack of: a new item with a new
/// object id is added (`addItem(process, itemId, count)`) and the pet's row is destroyed.
#[test]
fn a_stack_the_offline_owner_does_not_have_becomes_a_new_item() {
    let mut s = decayed_while_the_owner_is_offline();
    s.persisted.offline_pet_requests();
    s.world.handle(WorldCommand::OfflinePetRowsLoaded {
        owner: ME,
        control_object_id: CONTROL,
        result: Ok(OfflinePetRows {
            pet_items: vec![item(PET_ORE, ORE, 4, "PET")],
            inventory: vec![collar()],
        }),
    });
    let ops = s.persisted.item_row_ops();
    assert_eq!(ops.len(), 3, "{ops:?}");
    match &ops[0] {
        ItemRowOp::Insert { owner_id, item } => {
            assert_eq!(*owner_id, ME.0);
            assert_eq!((item.item_id, item.count, item.loc.as_str()), (ORE, 4, "INVENTORY"));
            // A new id of the Rust range, not the pet row's.
            assert!(item.object_id != PET_ORE && item.object_id.0 >= 0x6000_0000);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(ops[1], ItemRowOp::Delete { object_id: PET_ORE });
    assert_eq!(ops[2], ItemRowOp::Delete { object_id: CONTROL });
}

/// A failed read changes nothing: the items stay with the pet, the collar and its row stay.
#[test]
fn a_failed_read_leaves_the_offline_pets_rows_alone() {
    let mut s = decayed_while_the_owner_is_offline();
    s.persisted.offline_pet_requests();
    s.world.handle(WorldCommand::OfflinePetRowsLoaded {
        owner: ME,
        control_object_id: CONTROL,
        result: Err(()),
    });
    assert!(s.persisted.item_row_ops().is_empty());
    assert!(s.persisted.summon_writes().is_empty());
}

/// An owner who logged in again before the decay is told like any online owner: the items and the
/// collar are the ones of its new session.
#[test]
fn an_owner_who_is_back_online_gets_the_online_decay() {
    let mut s = Scene::new(vec![collar(), item(ORE_STACK, ORE, 10, "INVENTORY")]);
    let pet = s.use_collar();
    s.world.reduce_current_hp(pet, 100_000.0, Some(P2));
    s.input(ClientInput::Logout);
    // The same character enters again, the pet's ore among its rows.
    s.world.handle(WorldCommand::EnterWorld {
        session: SessionId(3),
        player: Box::new(player_data(
            ME,
            X,
            vec![
                collar(),
                item(ORE_STACK, ORE, 6, "INVENTORY"),
                item(PET_ORE, ORE, 4, "PET"),
            ],
            PlayerProfile::default(),
        )),
        sink: Box::new(Sink(s.me.clone())),
        ip: None,
    });
    s.advance(1000);
    s.persisted.summon_writes();
    s.me.take();
    s.advance(86_400_000 / 50 * 50 / 1000 * 1000 + 1000);
    // Online: no read, the owner's memory is changed and it hears about the collar.
    assert!(s.persisted.offline_pet_requests().is_empty());
    assert!(message_ids(&s.me.take()).contains(&SystemMessageId::S1HasDisappeared));
    assert_eq!(count_of(&s.world, ME, ORE, false), 10);
    assert!(
        s.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .all(|i| i.item_id != COLLAR)
    );
}
