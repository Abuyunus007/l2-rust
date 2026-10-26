#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The stale M6 markers (M6-FU-11): the summon branches of the duel, the GM pages of a pet, the
//! talisman mana of an item skill, `SkillCoolTime` where Java sends it, and the PC cafe points.
//! The expected values are read off the Java code named in each test.
//! Java: entity/actor/holders/player/Duel.java, entity/actor/status/{Player,Npc}Status.java,
//! entity/actor/Creature.java (doCast), entity/item/instance/Item.java (decreaseMana),
//! entity/itemcontainer/Inventory.java (the skill listeners), managers/PcCafePointsManager.java,
//! scripts/handlers/chat/commands/admin/{AdminEditChar,AdminBuffs}.java.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, ClassList, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates,
    KarmaLoss, NpcLoadOptions, NpcTemplates, PetDataTable, PetSkillData, PlayerTemplates, SkillTemplates,
    SkillTrees, SpawnData,
};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{
    ExPcCafePointInfo, GmViewItemList, NpcHtmlMessage, SkillCoolTime, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::duel::DuelState;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

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

// ---- the datapack ------------------------------------------------------------------------------

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

const WIND_STRIKE: i32 = 1177;
const TALISMAN_SKILL: i32 = 9501;
const BOOTS_SKILL: i32 = 9502;

const SKILLS: &str = r##"<list>
<skill id="2046" levels="2" name="Wolf's Necklace">
	<operateType>A1</operateType><targetType>SELF</targetType><hitTime>500</hitTime>
	<isMagic>2</isMagic><magicLevel>1</magicLevel><effects><effect name="SummonPet" /></effects>
</skill>
<skill id="1177" levels="1" name="Wind Strike">
	<operateType>A1</operateType><targetType>ONE</targetType><castRange>600</castRange>
	<effectPoint>-92</effectPoint><effectRange>1100</effectRange><element>2</element><elementPower>20</elementPower>
	<hitTime>4000</hitTime><isMagic>1</isMagic><magicLevel>1</magicLevel><mpConsume>7</mpConsume>
	<mpInitialConsume>2</mpInitialConsume><power>12</power><reuseDelay>1200</reuseDelay>
	<effects><effect name="MagicalDamage" /></effects>
</skill>
<skill id="9501" levels="1" name="Talisman Light"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>100</hitTime><abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel><isMagic>1</isMagic>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9502" levels="1" name="Boots Dash"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>100</hitTime><reuseDelay>5000</reuseDelay><abnormalTime>60</abnormalTime><abnormalType>SPEED_UP</abnormalType><abnormalLevel>1</abnormalLevel><isMagic>1</isMagic>
<effects><effect name="Buff"><mul stat="runSpd" val="1.1"/></effect></effects></skill>
</list>"##;

const COLLAR: i32 = 6648;
const TALISMAN: i32 = 9600;
const BOOTS: i32 = 9601;
const GLOVES: i32 = 9602;

const ITEMS: &str = r##"<list>
<item id="6648" type="EtcItem" name="Baby Buffalo Panpipe">
	<set name="default_action" val="SKILL_MAINTAIN" /><set name="etcitem_type" val="PET_COLLAR" />
	<set name="weight" val="20" /><set name="handler" val="SummonItems" />
	<skills><skill id="2046" level="1" /></skills>
</item>
<item id="9600" type="Armor" name="Test Talisman">
	<set name="bodypart" val="deco1" /><set name="armor_type" val="NONE" /><set name="weight" val="1" />
	<set name="useSkillDisTime" val="5" />
	<skills><skill id="9501" level="1" /></skills>
</item>
<item id="9601" type="Armor" name="Test Boots">
	<set name="bodypart" val="feet" /><set name="armor_type" val="LIGHT" /><set name="weight" val="5" />
	<set name="equip_reuse_delay" val="30" />
	<skills><skill id="9502" level="1" /></skills>
</item>
<item id="9602" type="Armor" name="Test Gloves">
	<set name="bodypart" val="gloves" /><set name="armor_type" val="LIGHT" /><set name="weight" val="5" />
	<set name="equip_reuse_delay" val="10" />
	<skills><skill id="9502" level="1" /></skills>
</item>
</list>"##;

const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="12780" level="3" type="BabyPet" name="Baby Buffalo">
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

/// Levels 1 to 6 of the pet: the exp each starts at is 0, 100, 300, 600, 1000, 2000.
fn pet_data() -> Arc<PetDataTable> {
    let level = |level: i32, exp: i64, hp: i32| {
        format!(
            r#"<stat level="{level}"><set name="max_meal" val="20"/><set name="exp" val="{exp}"/>
        <set name="get_exp_type" val="70"/><set name="consume_meal_in_battle" val="3"/>
        <set name="consume_meal_in_normal" val="2"/><set name="org_pattack" val="10"/>
        <set name="org_pdefend" val="20"/><set name="org_mattack" val="15"/><set name="org_mdefend" val="25"/>
        <set name="org_hp" val="{hp}"/><set name="org_mp" val="80"/><set name="org_hp_regen" val="6"/>
        <set name="org_mp_regen" val="3"/><set name="soulshot_count" val="2"/>
        <set name="spiritshot_count" val="1"/></stat>"#
        )
    };
    let levels: String = [
        (1, 0, 100),
        (2, 100, 120),
        (3, 300, 140),
        (4, 600, 160),
        (5, 1000, 180),
        (6, 2000, 200),
    ]
    .into_iter()
    .map(|(l, e, hp)| level(l, e, hp))
    .collect();
    Arc::new(
        PetDataTable::from_xml(&format!(
            r#"<pets>
            <pet id="12780" itemId="6648"><set name="food" val="7582"/><set name="hungry_limit" val="55"/>
            <set name="load" val="54510"/><skills><skill minLevel="1" skillId="4717" skillLevel="0"/></skills>
            <stats>{levels}</stats></pet></pets>"#
        ))
        .unwrap(),
    )
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

fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (100, 0)] {
        rights.add_level(level, child);
    }
    for command in ["summon_info", "summon_setlvl", "show_pet_inv", "removereuse"] {
        rights.add_command(command, 100, false);
    }
    rights
}

struct Data {
    stats: StatData,
    rights: AdminRights,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.stats.items.get(item_id)?;
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
    fn item_stackable(&self, item_id: i32) -> bool {
        self.stats.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.stats.items.get(item_id).map(|t| t.name.clone())
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
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
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

// ---- the scene ---------------------------------------------------------------------------------

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const FOE: ObjectId = ObjectId(0x2000_0002);
const GM: ObjectId = ObjectId(0x2000_0003);
const OTHER: ObjectId = ObjectId(0x2000_0004);
const COLLAR_OBJ: ObjectId = ObjectId(0x2100_0001);
const COLLAR_2_OBJ: ObjectId = ObjectId(0x2100_0002);

fn item(object_id: ObjectId, item_id: i32, loc: &str, loc_data: usize) -> ItemData {
    ItemData {
        object_id,
        item_id,
        count: 1,
        loc: loc.into(),
        loc_data: i32::try_from(loc_data).unwrap(),
        ..Default::default()
    }
}

fn player_data(id: ObjectId, name: &str, access_level: i32, dx: i32, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: name.into(),
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
        pk_kills: 0,
        access_level,
        x: X + dx,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 1000.0,
        max_hp: 1000.0,
        cur_mp: 500.0,
        max_mp: 250.0,
        cur_cp: 200.0,
        max_cp: 200.0,
        online_time: 0,
        items,
        skills: vec![SkillData {
            skill_id: WIND_STRIKE,
            skill_level: 1,
        }],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    /// Me (with a pet collar and `extra` items), the foe next to me, a GM, and `OTHER`, who has a
    /// collar too. `settings` is for the PC cafe.
    fn new(extra: Vec<ItemData>) -> Self {
        Self::with_pc_cafe(extra, PcCafeSettings::default())
    }

    fn with_pc_cafe(extra: Vec<ItemData>, pc_cafe: PcCafeSettings) -> Self {
        let root = tempfile::tempdir().unwrap();
        let page = root.path().join("data/html/admin/petinfo.htm");
        std::fs::create_dir_all(page.parent().unwrap()).unwrap();
        std::fs::write(
            &page,
            "<html><body>%name%|%level%|%exp%|%food%|%load%|%class%|%inv%</body></html>",
        )
        .unwrap();
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(NullOutbox));
        world.set_static_data(Box::new(Data {
            stats: stat_data(),
            rights: rights(),
        }));
        world.set_regen_settings(RegenSettings {
            hp_regen_multiplier: 0.0,
            mp_regen_multiplier: 0.0,
            cp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        let mut settings = WorldSettings::default();
        settings.vitality.enable_vitality = false;
        world.set_settings(settings);
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
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_class_list(Arc::new(
            ClassList::from_xml(r#"<list><class classId="0" name="Human Fighter" /></list>"#).unwrap(),
        ));
        world.set_pc_cafe_settings(pc_cafe);
        let mut scene = Self {
            world,
            log,
            _root: root,
        };
        let mut mine = vec![item(COLLAR_OBJ, COLLAR, "INVENTORY", 0)];
        mine.extend(extra);
        scene.enter(player_data(ME, "Alpha", 0, 0, mine));
        scene.enter(player_data(FOE, "Bravo", 0, 100, vec![]));
        scene.enter(player_data(GM, "Gamma", 100, 150, vec![]));
        scene.enter(player_data(
            OTHER,
            "Delta",
            0,
            120,
            vec![item(COLLAR_2_OBJ, COLLAR, "INVENTORY", 0)],
        ));
        scene.ticks(20);
        scene.log.take();
        scene
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id.0).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id.0, self.log.clone())),
            ip: None,
        });
    }

    fn input(&mut self, who: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    /// Double click on the collar: the pet is out a second later.
    fn summon_pet(&mut self, owner: ObjectId, collar: ObjectId) -> ObjectId {
        self.input(
            owner,
            ClientInput::UseItem {
                object_id: collar.0,
                ctrl_pressed: false,
            },
        );
        self.ticks(30);
        self.world.summon_of(owner).expect("a pet")
    }

    fn cast(&mut self, who: ObjectId, skill: i32) {
        self.input(
            who,
            ClientInput::MagicSkillUse {
                magic_id: skill,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        );
    }

    fn challenge_and_start(&mut self) {
        self.input(
            ME,
            ClientInput::DuelStart {
                name: "Bravo".into(),
                party_duel: 0,
            },
        );
        self.input(
            FOE,
            ClientInput::DuelAnswerStart {
                party_duel: 0,
                response: 1,
            },
        );
        for _ in 0..400 {
            self.ticks(1);
            if self.world.is_in_duel(ME) {
                return;
            }
        }
        panic!("the duel did not start");
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

fn names(sent: &Sent, to: ObjectId) -> Vec<String> {
    sent.iter()
        .filter(|(t, _)| *t == to.0)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn decoded<P: Decode>(sent: &Sent, to: ObjectId, name: &str) -> Vec<P> {
    sent.iter()
        .filter(|(t, b)| *t == to.0 && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

fn messages(sent: &Sent, to: ObjectId) -> Vec<SystemMessage> {
    decoded(sent, to, "SYSTEM_MESSAGE")
}

fn message_ids(sent: &Sent, to: ObjectId) -> Vec<SystemMessageId> {
    messages(sent, to).into_iter().map(|m| m.id).collect()
}

fn texts(sent: &Sent, to: ObjectId) -> Vec<String> {
    messages(sent, to)
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn admin(s: &mut Scene, command: &str) {
    s.world.admin_command(GM, command, false);
}

// ---- duel: the private store and the summons ----------------------------------------------------

/// `Player.canDuel`: "_privateStoreType != NONE" is its own refusal, with the name of the player
/// who is in the store.
#[test]
fn a_player_in_a_store_cannot_duel() {
    let mut s = Scene::new(vec![]);
    // The sell store window (`RequestActionUse` 10) puts the player in store mode.
    s.input(FOE, ClientInput::Store(StoreInput::ActionUse { action_id: 10 }));
    assert!(s.world.is_in_store_mode(FOE));
    assert_eq!(
        s.world.can_duel(FOE),
        Err(SystemMessageId::C1CannotDuelBecauseC1IsCurrentlyEngagedInAPrivateStoreOrManufacture)
    );
    s.log.take();
    s.input(
        ME,
        ClientInput::DuelStart {
            name: "Bravo".into(),
            party_duel: 0,
        },
    );
    let sent = s.log.take();
    let said = messages(&sent, ME);
    assert_eq!(
        said.iter().map(|m| m.id).collect::<Vec<_>>(),
        [SystemMessageId::C1CannotDuelBecauseC1IsCurrentlyEngagedInAPrivateStoreOrManufacture]
    );
    assert_eq!(said[0].params, [SystemMessageParam::PlayerName("Bravo".into())]);
    assert!(!s.world.is_in_duel(ME));
    // Nobody else is refused for it.
    assert_eq!(s.world.can_duel(ME), Ok(()));
}

/// `Player.setTeam`: "if (hasSummon()) _summon.broadcastStatusUpdate()", so the owner's pet window
/// is refreshed when the duel starts.
#[test]
fn the_duel_team_refreshes_the_pet_of_the_duelist() {
    let mut s = Scene::new(vec![]);
    s.summon_pet(ME, COLLAR_OBJ);
    s.ticks(40);
    s.log.take();
    s.challenge_and_start();
    let _ = s.log.take();
    // A fresh log from the moment of the start: finish the duel and see the team go.
    s.input(FOE, ClientInput::DuelSurrender);
    s.ticks(200);
    let sent = s.log.take();
    // `setTeam(NONE)` at the end of the duel refreshed the pet window of the owner.
    assert!(
        names(&sent, ME).iter().any(|n| n == "PET_INFO"),
        "{:?}",
        names(&sent, ME)
    );
}

/// `Duel.stopFighting`: a living summon of the duelist is stopped too ("abortCast, setIntentionActive,
/// setTarget(null), ActionFailed"); its `ActionFailed` reaches the owner.
#[test]
fn stopping_a_duel_stops_the_summon_too() {
    fn action_fails(with_pet: bool) -> usize {
        let mut s = Scene::new(vec![]);
        if with_pet {
            s.summon_pet(ME, COLLAR_OBJ);
            s.ticks(40);
        }
        s.challenge_and_start();
        s.log.take();
        s.input(FOE, ClientInput::DuelSurrender);
        let sent = s.log.take();
        names(&sent, ME).iter().filter(|n| *n == "ACTION_FAIL").count()
    }
    assert_eq!(action_fails(false) + 1, action_fails(true));
}

/// `PlayerStatus.reduceHp`: "attackerPlayer = attacker.asPlayer()", so a duelist who has won is not
/// hurt by the pet of the other duelist either.
#[test]
fn the_pet_of_a_duelist_counts_as_its_owner_when_it_hits() {
    let mut s = Scene::new(vec![]);
    let pet = s.summon_pet(ME, COLLAR_OBJ);
    s.challenge_and_start();
    s.world.set_duel_state(FOE, DuelState::Winner);
    let before = {
        let p = s.world.player(FOE).unwrap();
        (p.core.cur_cp, p.core.cur_hp)
    };
    s.world.reduce_current_hp(FOE, 100.0, Some(pet));
    let p = s.world.player(FOE).unwrap();
    assert_eq!((p.core.cur_cp, p.core.cur_hp), before, "the damage is ignored");
    assert_eq!(s.world.duel_state_of(FOE), DuelState::Winner);
}

/// `NpcStatus.reduceHp`: a duelist whose summon hits a monster leaves the duel.
#[test]
fn the_pet_of_a_duelist_hitting_a_monster_interrupts_the_duel() {
    let mut s = Scene::new(vec![]);
    let pet = s.summon_pet(ME, COLLAR_OBJ);
    s.challenge_and_start();
    assert_eq!(s.world.duel_state_of(ME), DuelState::Duelling);
    let monster = s.world.spawn_npc_at(9001, X, Y, Z, 0).unwrap();
    s.world.reduce_current_hp(monster, 5.0, Some(pet));
    assert_eq!(s.world.duel_state_of(ME), DuelState::Interrupted);
}

/// `Player.checkUseMagicConditions`: "if (_isInDuel && target.isPlayable())" the target (a summon is
/// its owner's) has to be in the same duel.
#[test]
fn a_duelist_cannot_cast_at_the_pet_of_somebody_outside_the_duel() {
    let mut s = Scene::new(vec![]);
    let own = s.summon_pet(ME, COLLAR_OBJ);
    let other = s.summon_pet(OTHER, COLLAR_2_OBJ);
    s.challenge_and_start();
    let refusal = SystemMessage::text("You cannot do this while duelling.").to_bytes();
    let refused = |sent: &Sent| {
        sent.iter()
            .any(|(to, b)| *to == ME.0 && name_of(b) == "SYSTEM_MESSAGE" && *b == refusal)
    };

    // At the pet of a bystander: refused, nothing is cast.
    s.world.set_target(ME, Some(other));
    s.log.take();
    s.cast(ME, WIND_STRIKE);
    let sent = s.log.take();
    assert!(refused(&sent), "{:?}", names(&sent, ME));
    assert_eq!(names(&sent, ME), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);

    // At the own pet: the same duel, no refusal.
    s.world.set_target(ME, Some(own));
    s.log.take();
    s.cast(ME, WIND_STRIKE);
    assert!(!refused(&s.log.take()));
}

/// `SetPrivateStoreListSell`: "hasAttackStanceTask(player) || player.isInDuel()" refuses the store
/// with the combat message and sends the manage window again.
#[test]
fn a_duelist_cannot_set_up_a_store() {
    let mut s = Scene::new(vec![]);
    s.challenge_and_start();
    s.log.take();
    s.input(
        ME,
        ClientInput::Store(StoreInput::ListSell {
            package_sale: false,
            items: Some(vec![]),
        }),
    );
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::WhileYouAreEngagedInCombatYouCannotOperateAPrivateStoreOrPrivateWorkshop]
    );
    assert_ne!(s.world.private_store_type(ME), PrivateStoreType::Sell);
}

/// `SummonStatus.reduceHp`: whoever hits a summon leaves his duel unless the summon's owner is in
/// the same duel.
#[test]
fn hitting_a_pet_outside_the_duel_interrupts_the_duel() {
    let mut s = Scene::new(vec![]);
    let own = s.summon_pet(ME, COLLAR_OBJ);
    let other = s.summon_pet(OTHER, COLLAR_2_OBJ);
    s.challenge_and_start();
    // The opponent hits the pet of the duelist: the same duel, the duel goes on.
    s.world.reduce_current_hp(own, 5.0, Some(FOE));
    assert_eq!(s.world.duel_state_of(FOE), DuelState::Duelling);
    // The duelist hits the pet of a bystander: his duel is interrupted.
    s.world.reduce_current_hp(other, 5.0, Some(ME));
    assert_eq!(s.world.duel_state_of(ME), DuelState::Interrupted);
}

// ---- the GM pages of a pet ---------------------------------------------------------------------

fn page(sent: &Sent) -> String {
    decoded::<NpcHtmlMessage>(sent, GM, "NPC_HTML_MESSAGE")
        .pop()
        .map(|m| m.html)
        .unwrap_or_default()
}

/// `AdminEditChar.gatherSummonInfo`: a pet shows "fed/max fed" and "weight/max load", a servitor
/// "N/A" (checked in `admin_social.rs`).
#[test]
fn the_pet_page_shows_food_and_load() {
    let mut s = Scene::new(vec![]);
    let pet = s.summon_pet(ME, COLLAR_OBJ);
    s.world.set_target(GM, Some(pet));
    s.log.take();
    admin(&mut s, "admin_summon_info");
    let html = page(&s.log.take());
    let max_load = s.world.stats(pet).unwrap().max_load();
    assert!(max_load > 0);
    // `%name%|%level%|%exp%|%food%|%load%|%class%|%inv%`: the pet has no name yet.
    assert_eq!(
        html,
        format!(
            "<html><body>N/A|3|300|20/20|0/{max_load}|Pet| <a action=\"bypass admin_show_pet_inv {}\">view</a></body></html>",
            ME.0
        )
    );
}

/// `admin_summon_setlvl`: the exp goes to the exp of the level, up (`addExp`) or down
/// (`removeExp`); the level follows the exp.
#[test]
fn summon_setlvl_moves_the_pet_to_the_level() {
    let mut s = Scene::new(vec![]);
    let pet = s.summon_pet(ME, COLLAR_OBJ);
    s.world.set_target(GM, Some(pet));
    assert_eq!(s.world.summon(pet).unwrap().exp, 300);

    // `getExpForLevel(5)` is 1000: `addExp(1000 - 300)`.
    admin(&mut s, "admin_summon_setlvl 5");
    assert_eq!(s.world.summon(pet).unwrap().exp, 1000);
    assert_eq!(s.world.stats(pet).unwrap().level(), 5);

    // `getExpForLevel(2)` is 100: `removeExp(1000 - 100)`.
    admin(&mut s, "admin_summon_setlvl 2");
    assert_eq!(s.world.summon(pet).unwrap().exp, 100);
    assert_eq!(s.world.stats(pet).unwrap().level(), 2);

    // The same level changes nothing; a word that is no number only logs.
    admin(&mut s, "admin_summon_setlvl 2");
    admin(&mut s, "admin_summon_setlvl x");
    assert_eq!(s.world.summon(pet).unwrap().exp, 100);
}

/// `admin_show_pet_inv`: `GMViewItemList(pet)`: the name of the pet, `INVENTORY_MAXIMUM_PET` (12)
/// and the items of the pet.
#[test]
fn show_pet_inv_sends_the_pet_item_window() {
    let mut s = Scene::new(vec![]);
    let pet = s.summon_pet(ME, COLLAR_OBJ);
    s.world.set_target(GM, Some(pet));
    s.log.take();
    admin(&mut s, "admin_show_pet_inv");
    let lists = decoded::<GmViewItemList>(&s.log.take(), GM, "GM_VIEW_ITEM_LIST");
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].limit, 12);
    assert!(lists[0].items.is_empty());

    // By the object id of the pet, with the target on somebody else.
    s.world.set_target(GM, Some(ME));
    admin(&mut s, &format!("admin_show_pet_inv {}", pet.0));
    assert_eq!(
        decoded::<GmViewItemList>(&s.log.take(), GM, "GM_VIEW_ITEM_LIST").len(),
        1
    );
}

// ---- the talisman ------------------------------------------------------------------------------

fn talisman(mana: i32) -> ItemData {
    ItemData {
        mana_left: mana,
        ..item(ObjectId(0x2100_0010), TALISMAN, "PAPERDOLL", paperdoll::DECO1)
    }
}

/// Whether the player has the skill (Java `getKnownSkill`).
fn knows(s: &Scene, skill_id: i32) -> bool {
    s.world
        .player(ME)
        .unwrap()
        .core
        .skills
        .iter()
        .any(|(id, _)| *id == skill_id)
}

fn mana_of(s: &Scene) -> Option<i32> {
    s.world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.item_id == TALISMAN)
        .map(|i| i.mana_left)
}

/// `Creature.doCast`: "Reduce talisman mana on skill use" by `useSkillDisTime` (5 here) for the
/// worn DECO item that carries the skill. `Item.decreaseMana` warns at 10, 5 and 1 and at 0 the
/// talisman is taken off and destroyed.
#[test]
fn a_cast_costs_the_talisman_mana() {
    let mut s = Scene::new(vec![talisman(15)]);
    assert_eq!(mana_of(&s), Some(15));
    // The worn talisman gave the skill.
    assert!(knows(&s, TALISMAN_SKILL));

    s.cast(ME, TALISMAN_SKILL);
    s.ticks(40);
    assert_eq!(mana_of(&s), Some(10));
    let sent = s.log.take();
    let said = messages(&sent, ME);
    let ten: Vec<_> = said
        .iter()
        .filter(|m| m.id == SystemMessageId::S1SRemainingManaIsNow10)
        .collect();
    assert_eq!(ten.len(), 1);
    assert_eq!(ten[0].params, [SystemMessageParam::ItemName(TALISMAN)]);

    s.world
        .stop_skill_effects(ME, SkillFinishType::Normal, TALISMAN_SKILL);
    s.cast(ME, TALISMAN_SKILL);
    s.ticks(40);
    assert_eq!(mana_of(&s), Some(5));
    assert!(message_ids(&s.log.take(), ME).contains(&SystemMessageId::S1SRemainingManaIsNow5));

    // The last 5: "remaining mana is now 0 and the item has disappeared", off and gone.
    s.world
        .stop_skill_effects(ME, SkillFinishType::Normal, TALISMAN_SKILL);
    s.cast(ME, TALISMAN_SKILL);
    s.ticks(40);
    assert_eq!(mana_of(&s), None);
    let sent = s.log.take();
    assert!(
        message_ids(&sent, ME).contains(&SystemMessageId::S1SRemainingManaIsNow0AndTheItemHasDisappeared)
    );
    let names = names(&sent, ME);
    assert!(names.iter().any(|n| n == "INVENTORY_UPDATE"), "{names:?}");
    assert!(
        s.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .all(|i| i.item_id != TALISMAN)
    );
}

/// A talisman with less mana than the cast costs aborts the cast and keeps its mana.
#[test]
fn a_talisman_without_enough_mana_aborts_the_cast() {
    let mut s = Scene::new(vec![talisman(3)]);
    s.cast(ME, TALISMAN_SKILL);
    s.ticks(40);
    assert_eq!(mana_of(&s), Some(3));
    assert!(!s.world.is_affected_by_skill(ME, TALISMAN_SKILL));
}

// ---- SkillCoolTime -----------------------------------------------------------------------------

fn boots_in_the_bag() -> ItemData {
    item(ObjectId(0x2100_0020), BOOTS, "INVENTORY", 0)
}

/// `Inventory.ItemSkillsListener.notifyEquiped`: an active item skill gets the item's
/// `equip_reuse_delay` (30 s) as its reuse, and "if (update) { sendSkillList();
/// if (updateTimeStamp) sendPacket(new SkillCoolTime(player)); }" (the list itself is queued).
#[test]
fn equipping_an_item_with_an_active_skill_sends_the_cool_time() {
    let mut s = Scene::new(vec![boots_in_the_bag()]);
    s.log.take();
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: 0x2100_0020,
            ctrl_pressed: false,
        },
    );
    // `sendSkillList()` is queued for 300 ms, so the cool times reach the client first.
    s.ticks(10);
    let sent = s.log.take();
    let order = names(&sent, ME);
    let list = order
        .iter()
        .position(|n| n == "SKILL_LIST")
        .unwrap_or_else(|| panic!("{order:?}"));
    let cool = order
        .iter()
        .position(|n| n == "SKILL_COOL_TIME")
        .expect("a cool time packet");
    assert!(cool < list, "{order:?}");
    let packet = decoded::<SkillCoolTime>(&sent, ME, "SKILL_COOL_TIME")
        .pop()
        .unwrap();
    assert_eq!(packet.entries.len(), 1);
    let entry = &packet.entries[0];
    assert_eq!((entry.skill_id, entry.level), (BOOTS_SKILL, 1));
    assert_eq!((entry.reuse_seconds, entry.remaining_seconds), (30, 30));
    let hash = stat_data().skills.get(BOOTS_SKILL, 1).unwrap().hash_code();
    assert!(s.world.has_skill_reuse(ME, hash));
}

/// `ItemSkillsListener.notifyUnequiped`: the skill of another worn armor that the taken off one
/// shared comes back, and the client gets the cool times with the skill list.
#[test]
fn taking_off_an_armor_gives_back_the_skill_of_another_one() {
    let worn_boots = item(ObjectId(0x2100_0020), BOOTS, "PAPERDOLL", paperdoll::FEET);
    let worn_gloves = item(ObjectId(0x2100_0021), GLOVES, "PAPERDOLL", paperdoll::GLOVES);
    let mut s = Scene::new(vec![worn_boots, worn_gloves]);
    assert!(knows(&s, BOOTS_SKILL));
    s.log.take();
    // The gloves come off: their skill (the same 9502) goes, the boots give it back.
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: 0x2100_0021,
            ctrl_pressed: false,
        },
    );
    s.ticks(10);
    let sent = s.log.take();
    let order = names(&sent, ME);
    let list = order
        .iter()
        .position(|n| n == "SKILL_LIST")
        .unwrap_or_else(|| panic!("{order:?}"));
    let cool = order
        .iter()
        .position(|n| n == "SKILL_COOL_TIME")
        .unwrap_or_else(|| panic!("{order:?}"));
    assert!(cool < list, "{order:?}");
    assert!(knows(&s, BOOTS_SKILL));
}

/// `//removereuse`: "player.resetTimeStamps(); player.resetDisabledSkills();
/// player.sendPacket(new SkillCoolTime(player))", so the client's icons clear.
#[test]
fn removereuse_sends_the_empty_cool_time_list() {
    let mut s = Scene::new(vec![]);
    let skills = stat_data().skills;
    s.world
        .add_time_stamp(ME, skills.get(WIND_STRIKE, 1).unwrap(), 60_000);
    assert_eq!(s.world.skill_cool_time(ME).entries.len(), 1);
    s.world.set_target(GM, Some(ME));
    s.log.take();
    admin(&mut s, "admin_removereuse");
    let sent = s.log.take();
    let packets = decoded::<SkillCoolTime>(&sent, ME, "SKILL_COOL_TIME");
    assert_eq!(packets.len(), 1);
    assert!(packets[0].entries.is_empty());
    assert_eq!(texts(&sent, GM), ["Skill reuse was removed from Alpha."]);
}

// ---- PC cafe points ----------------------------------------------------------------------------

fn hunting(rate: f64) -> PcCafeSettings {
    PcCafeSettings {
        enabled: true,
        retail_like: false,
        only_premium: false,
        point_rate: rate,
        ..PcCafeSettings::default()
    }
}

fn info_of(sent: &Sent) -> Vec<ExPcCafePointInfo> {
    decoded(sent, ME, "EX_PC_CAFE_POINT_INFO")
}

/// `givePcCafePoint`: "(int) (exp * 0.0001 * PC_CAFE_POINT_RATE)" points, the message, the points
/// and `ExPCCafePointInfo(points, gained, 0)`.
#[test]
fn hunting_gives_points_for_the_exp() {
    let mut s = Scene::with_pc_cafe(vec![], hunting(1.0));
    s.world.give_pc_cafe_point(ME, 50_000.0);
    let sent = s.log.take();
    assert_eq!(s.world.pc_cafe_points(ME), 5);
    let said = messages(&sent, ME);
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].id, SystemMessageId::YouHaveAcquiredS1PcCafePoints);
    assert_eq!(said[0].params, [SystemMessageParam::LongNumber(5)]);
    assert_eq!(info_of(&sent), [ExPcCafePointInfo::new(5, 5, 0)]);
    // The message comes before the packet.
    let order = names(&sent, ME);
    assert_eq!(order, ["SYSTEM_MESSAGE", "EX_PC_CAFE_POINT_INFO"]);

    // The rate multiplies.
    let mut s = Scene::with_pc_cafe(vec![], hunting(2.5));
    s.world.give_pc_cafe_point(ME, 50_000.0);
    assert_eq!(s.world.pc_cafe_points(ME), 12);
}

/// The kills that are worth less than one point: "RewardLowExpKills" gives 1 point with the
/// chance `RewardLowExpKillsChance` (`Rnd.get(100) < chance`).
#[test]
fn a_low_exp_kill_gives_one_point_by_chance() {
    let sure = PcCafeSettings {
        low_exp_kills_chance: 100,
        ..hunting(1.0)
    };
    let mut s = Scene::with_pc_cafe(vec![], sure);
    s.world.give_pc_cafe_point(ME, 100.0);
    assert_eq!(s.world.pc_cafe_points(ME), 1);

    let never = PcCafeSettings {
        low_exp_kills_chance: 0,
        ..hunting(1.0)
    };
    let mut s = Scene::with_pc_cafe(vec![], never);
    s.world.give_pc_cafe_point(ME, 100.0);
    assert_eq!(s.world.pc_cafe_points(ME), 0);
    assert!(s.log.take().is_empty());

    let off = PcCafeSettings {
        reward_low_exp_kills: false,
        low_exp_kills_chance: 100,
        ..hunting(1.0)
    };
    let mut s = Scene::with_pc_cafe(vec![], off);
    s.world.give_pc_cafe_point(ME, 100.0);
    assert_eq!(s.world.pc_cafe_points(ME), 0);
}

/// Nothing is given when the feature is off, when the retail timer is the source, or for a premium
/// only server (nobody is premium here).
#[test]
fn hunting_gives_nothing_when_the_settings_say_so() {
    for settings in [
        PcCafeSettings::default(),
        PcCafeSettings {
            retail_like: true,
            ..hunting(1.0)
        },
        PcCafeSettings {
            only_premium: true,
            ..hunting(1.0)
        },
    ] {
        let mut s = Scene::with_pc_cafe(vec![], settings);
        s.log.take();
        s.world.give_pc_cafe_point(ME, 1_000_000.0);
        assert_eq!(s.world.pc_cafe_points(ME), 0);
        assert!(s.log.take().is_empty());
    }
}

/// `MaxPcCafePoints`: a gift is cut to what fits, and at the top "the maximum accumulation ... has
/// been exceeded" and nothing is given.
#[test]
fn the_points_stop_at_the_maximum() {
    let settings = PcCafeSettings {
        max_points: 10,
        ..hunting(1.0)
    };
    let mut s = Scene::with_pc_cafe(vec![], settings);
    s.world.set_pc_cafe_points(ME, 8);
    s.world.give_pc_cafe_point(ME, 50_000.0);
    let sent = s.log.take();
    assert_eq!(s.world.pc_cafe_points(ME), 10);
    assert_eq!(messages(&sent, ME)[0].params, [SystemMessageParam::LongNumber(2)]);
    assert_eq!(info_of(&sent), [ExPcCafePointInfo::new(10, 2, 0)]);

    s.world.give_pc_cafe_point(ME, 50_000.0);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::TheMaximumAccumulationAllowedOfPcCafePointsHasBeenExceededYouCanNoLongerAcquirePcCafePoints]
    );
    assert!(info_of(&sent).is_empty());
    // `setPcCafePoints` itself never goes above the maximum.
    s.world.set_pc_cafe_points(ME, 500);
    assert_eq!(s.world.pc_cafe_points(ME), 10);
}

/// `DoublingAcquisitionPoints`: with the chance at 100 every gift is doubled, with the other
/// message.
#[test]
fn double_points_have_their_own_message() {
    let settings = PcCafeSettings {
        enable_double_points: true,
        double_points_chance: 100,
        ..hunting(1.0)
    };
    let mut s = Scene::with_pc_cafe(vec![], settings);
    s.world.give_pc_cafe_point(ME, 50_000.0);
    let sent = s.log.take();
    assert_eq!(s.world.pc_cafe_points(ME), 10);
    let said = messages(&sent, ME);
    assert_eq!(said[0].id, SystemMessageId::DoublePointsYouAcquiredS1PcBangPoint);
    assert_eq!(said[0].params, [SystemMessageParam::LongNumber(10)]);
}

/// `giveRetailPcCafePont`: every `PcCafeRewardTime` the player gets `AcquisitionPointsRetailLikePoints`,
/// with `ExPCCafePointInfo(points, gained, 1)` (the time argument is 1: 3 on the wire).
#[test]
fn the_retail_timer_gives_points_every_period() {
    let settings = PcCafeSettings {
        enabled: true,
        retail_like: true,
        reward_time_ms: 1000,
        retail_like_points: 10,
        ..PcCafeSettings::default()
    };
    let mut s = Scene::with_pc_cafe(vec![], settings);
    // `Scene::new` ran 20 ticks (1 s) after the players entered: the first period has passed.
    assert_eq!(s.world.pc_cafe_points(ME), 10);
    s.log.take();
    s.ticks(20);
    let sent = s.log.take();
    assert_eq!(s.world.pc_cafe_points(ME), 20);
    assert_eq!(info_of(&sent), [ExPcCafePointInfo::new(20, 10, 1)]);
    assert_eq!(
        message_ids(&sent, ME),
        [SystemMessageId::YouHaveAcquiredS1PcCafePoints]
    );
    // Hunting gives nothing in this mode, and a player who left gets no more.
    s.world.give_pc_cafe_point(ME, 1_000_000.0);
    assert_eq!(s.world.pc_cafe_points(ME), 20);
}

/// The timer is not started when the feature or the retail mode is off.
#[test]
fn no_retail_timer_without_the_settings() {
    let mut s = Scene::with_pc_cafe(vec![], hunting(1.0));
    s.ticks(100);
    assert_eq!(s.world.pc_cafe_points(ME), 0);
    let off = PcCafeSettings {
        retail_like: true,
        reward_time_ms: 1000,
        ..PcCafeSettings::default()
    };
    let mut s = Scene::with_pc_cafe(vec![], off);
    s.ticks(100);
    assert_eq!(s.world.pc_cafe_points(ME), 0);
}
