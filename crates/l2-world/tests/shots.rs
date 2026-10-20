#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Soulshots and spiritshots (M5-SHOT-01): the three item handlers, the charge on the weapon,
//! the automatic use (`RequestAutoSoulShot`, `rechargeShots` after a hit and on equipping),
//! grade checks and messages, the soulshot flag and damage of an attack, the 0.6 cast time of
//! spiritshots, NPC shots, and the shortcut rule. Expected behaviour comes from the Java code
//! named in each test (`SoulShots.java`, `SpiritShot.java`, `BlessedSpiritShot.java`,
//! `Player.rechargeShots`, `Npc.rechargeShots`, `RequestAutoSoulShot.java`).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{Attack, ExAutoSoulShot, MagicSkillUse, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
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

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

fn names(sent: &Sent) -> Vec<String> {
    sent.iter().map(|b| name_of(b)).collect()
}

fn of<'a>(sent: &'a Sent, name: &str) -> Vec<&'a [u8]> {
    sent.iter()
        .filter(|b| name_of(b) == name)
        .map(Vec::as_slice)
        .collect()
}

/// The ids of the system messages, in order.
fn messages(sent: &Sent) -> Vec<SystemMessageId> {
    of(sent, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw).id)
        .collect()
}

fn auto_shot_packets(sent: &Sent) -> Vec<(i32, i32)> {
    of(sent, "EX_AUTO_SOULSHOT")
        .iter()
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
const SWORD_D: i32 = 3;
const STAFF_NG: i32 = 4;
const SOULSHOT_NG: i32 = 1835;
const SOULSHOT_D: i32 = 1463;
const SPIRITSHOT_NG: i32 = 2509;
const BLESSED_NG: i32 = 3947;
const SPIRITSHOT_D: i32 = 2510;

/// Weapons: a no-grade sword that uses 2 soulshots (and has no random damage), a D-grade sword
/// that uses 1, a no-grade staff that uses 1 spiritshot and 3 reduced soulshots... and the shots.
const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<set name="soulshots" val="2" /><set name="spiritshots" val="2" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
<item id="3" type="Weapon" name="D Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<set name="crystal_type" val="D" /><set name="soulshots" val="3" /><set name="spiritshots" val="1" />
<set name="reduced_soulshot" val="100,1" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
<item id="4" type="Weapon" name="Staff"><set name="weapon_type" val="BLUNT" /><set name="bodypart" val="rhand" />
<set name="soulshots" val="0" /><set name="spiritshots" val="1" /><set name="is_magic_weapon" val="true" />
<stats><stat type="pAtk">10</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">325</stat><stat type="pAtkRange">40</stat></stats></item>
<item id="1835" type="EtcItem" name="Soulshot: No Grade"><set name="default_action" val="SOULSHOT" />
<set name="is_stackable" val="true" /><set name="handler" val="SoulShots" /><skills><skill id="2039" level="1" /></skills></item>
<item id="1463" type="EtcItem" name="Soulshot: D-grade"><set name="default_action" val="SOULSHOT" /><set name="crystal_type" val="D" />
<set name="is_stackable" val="true" /><set name="handler" val="SoulShots" /><skills><skill id="2150" level="1" /></skills></item>
<item id="2509" type="EtcItem" name="Spiritshot: No Grade"><set name="default_action" val="SPIRITSHOT" />
<set name="is_stackable" val="true" /><set name="handler" val="SpiritShot" /><skills><skill id="2061" level="1" /></skills></item>
<item id="2510" type="EtcItem" name="Spiritshot: D-grade"><set name="default_action" val="SPIRITSHOT" /><set name="crystal_type" val="D" />
<set name="is_stackable" val="true" /><set name="handler" val="SpiritShot" /><skills><skill id="2155" level="1" /></skills></item>
<item id="3947" type="EtcItem" name="Blessed Spiritshot: No Grade"><set name="default_action" val="SPIRITSHOT" />
<set name="is_stackable" val="true" /><set name="handler" val="BlessedSpiritShot" /><skills><skill id="2061" level="1" /></skills></item>
<item id="6535" type="EtcItem" name="Fishing Shot: No Grade"><set name="is_stackable" val="true" /></item>
<item id="6645" type="EtcItem" name="Blessed Spiritshot for Beasts"><set name="default_action" val="SUMMON_SPIRITSHOT" />
<set name="is_stackable" val="true" /></item>
</list>"#;

const SKILLS: &str = r##"<list>
<skill id="1177" levels="1" name="Wind Strike"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>-92</effectPoint><effectRange>1100</effectRange>
<hitTime>4000</hitTime><isMagic>1</isMagic><magicLevel>1</magicLevel><mpConsume>7</mpConsume>
<power>12</power><effects><effect name="MagicalDamage" /></effects></skill>
<skill id="9006" levels="1" name="Power Strike"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>40</castRange><effectRange>400</effectRange><effectPoint>-100</effectPoint>
<hitTime>1080</hitTime><isMagic>0</isMagic><mpConsume>3</mpConsume>
<effects><effect name="PhysicalDamage"><param power="25"/></effect></effects></skill>
<skill id="9007" levels="1" name="Shield Up"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>1000</hitTime><isMagic>0</isMagic><abnormalTime>60</abnormalTime><abnormalType>PD_UP</abnormalType>
<abnormalLevel>1</abnormalLevel><effects><effect name="Buff"><mul stat="pDef" val="1.1"/></effect></effects></skill>
<skill id="9002" levels="1" name="Empower"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>1000</hitTime><isMagic>1</isMagic><mpConsume>1</mpConsume>
<abnormalTime>60</abnormalTime><abnormalType>MA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="mAtk" val="1.1"/></effect></effects></skill>
</list>"##;

/// A monster with soulshots: 3 of them, always (chance 100).
const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="9001" type="Monster" level="1" name="Weak Monster">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="30000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9002" type="Monster" level="1" name="Shooter">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="30000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <shots soul="3" spirit="0" shotChance="100" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
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
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const SWORD_OBJ: ObjectId = ObjectId(0x3000_0001);
const SHOTS_OBJ: ObjectId = ObjectId(0x3000_0002);

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

fn worn(item_id: i32) -> ItemData {
    item(SWORD_OBJ, item_id, "PAPERDOLL", paperdoll::RHAND, 1)
}

fn bag(item_id: i32, count: i64) -> ItemData {
    item(SHOTS_OBJ, item_id, "INVENTORY", 0, count)
}

fn player_data(items: Vec<ItemData>, shortcuts: Vec<ShortcutData>) -> PlayerData {
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
        skills: vec![SkillData {
            skill_id: 9002,
            skill_level: 1,
        }],
        shortcuts,
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new(items: Vec<ItemData>) -> Self {
        Self::with_shortcuts(items, vec![])
    }

    fn with_shortcuts(items: Vec<ItemData>, shortcuts: Vec<ShortcutData>) -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(20_261_020), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(items, shortcuts)),
            sink: Box::new(Sink(log.clone())),
            ip: None,
        });
        let mut scene = Self { world, log };
        scene.ticks(20);
        scene.log.take();
        scene
    }

    fn input(&mut self, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input,
        });
    }

    fn use_item(&mut self, object_id: ObjectId) -> Sent {
        self.input(ClientInput::UseItem {
            object_id: object_id.0,
            ctrl_pressed: false,
        });
        self.log.take()
    }

    fn auto(&mut self, item_id: i32, on: bool) -> Sent {
        self.input(ClientInput::AutoSoulShot {
            item_id,
            shot_type: i32::from(on),
        });
        self.log.take()
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn count_of(&self, item_id: i32) -> i64 {
        self.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .find(|i| i.item_id == item_id)
            .map_or(0, |i| i.count)
    }

    fn charged(&self, shot: ShotType) -> bool {
        self.world.is_charged_shot(ME, shot)
    }
}

// ---- the handlers ------------------------------------------------------------------------------

/// `SoulShots.onItemUse`: the weapon uses 2, so 2 are consumed; the charge is set, the client gets
/// "Your soulshots are enabled." and the shot animation (the skill of the item, to itself); a
/// second use does nothing while the weapon is charged.
#[test]
fn soulshot_charges_the_weapon_and_consumes_the_weapons_count() {
    let mut s = Scene::new(vec![worn(SWORD_NG), bag(SOULSHOT_NG, 10)]);
    assert!(!s.charged(ShotType::Soulshots));
    let sent = s.use_item(SHOTS_OBJ);
    assert_eq!(s.count_of(SOULSHOT_NG), 8);
    assert!(s.charged(ShotType::Soulshots));
    assert!(!s.charged(ShotType::Spiritshots));
    assert_eq!(messages(&sent), vec![SystemMessageId::YourSoulshotsAreEnabled]);
    let animation = decode::<MagicSkillUse>(of(&sent, "MAGIC_SKILL_USE")[0]);
    assert_eq!(
        (animation.creature_id, animation.target_id),
        (ME.0, ME.0),
        "the player uses it on itself"
    );
    assert_eq!((animation.skill_id, animation.skill_level), (2039, 1));
    assert_eq!((animation.hit_time, animation.reuse_delay), (0, 0));

    // Already charged: nothing happens, nothing is consumed, no message.
    let sent = s.use_item(SHOTS_OBJ);
    assert_eq!(s.count_of(SOULSHOT_NG), 8);
    assert!(messages(&sent).is_empty(), "{:?}", names(&sent));
    assert!(of(&sent, "MAGIC_SKILL_USE").is_empty());
}

/// Without a weapon (fists use no shots), or with a weapon that uses none of the kind, the
/// "cannot use" message comes, unless the shot is automatic.
#[test]
fn shots_need_a_weapon_that_uses_them() {
    let mut s = Scene::new(vec![
        bag(SOULSHOT_NG, 10),
        item(ObjectId(0x3000_0003), SPIRITSHOT_NG, "INVENTORY", 1, 10),
    ]);
    let sent = s.use_item(SHOTS_OBJ);
    assert_eq!(messages(&sent), vec![SystemMessageId::CannotUseSoulshots]);
    assert_eq!(s.count_of(SOULSHOT_NG), 10);
    let sent = s.use_item(ObjectId(0x3000_0003));
    assert_eq!(messages(&sent), vec![SystemMessageId::YouMayNotUseSpiritshots]);

    // A staff has no soulshot count.
    let mut s = Scene::new(vec![worn(STAFF_NG), bag(SOULSHOT_NG, 10)]);
    let sent = s.use_item(SHOTS_OBJ);
    assert_eq!(messages(&sent), vec![SystemMessageId::CannotUseSoulshots]);
    assert!(!s.charged(ShotType::Soulshots));
}

/// The grade of the shot must be the weapon's: the message differs for soul and spirit shots, and
/// nothing is consumed or charged. An automatic shot fails silently.
#[test]
fn grade_mismatch_is_refused_and_silent_for_automatic_shots() {
    let mut s = Scene::new(vec![
        worn(SWORD_NG),
        bag(SOULSHOT_D, 10),
        item(ObjectId(0x3000_0003), SPIRITSHOT_D, "INVENTORY", 1, 10),
    ]);
    let sent = s.use_item(SHOTS_OBJ);
    assert_eq!(
        messages(&sent),
        vec![SystemMessageId::TheSoulshotYouAreAttemptingToUseDoesNotMatchTheGradeOfYourEquippedWeapon]
    );
    let sent = s.use_item(ObjectId(0x3000_0003));
    assert_eq!(
        messages(&sent),
        vec![SystemMessageId::YourSpiritshotDoesNotMatchTheWeaponSGrade]
    );
    assert_eq!(s.count_of(SOULSHOT_D), 10);
    assert_eq!(s.count_of(SPIRITSHOT_D), 10);
    assert!(!s.charged(ShotType::Soulshots) && !s.charged(ShotType::Spiritshots));

    // Switched on at the request, the wrong grade is announced there (`RequestAutoSoulShot`) and
    // then stays silent when `rechargeShots` tries it.
    let sent = s.auto(SOULSHOT_D, true);
    assert_eq!(
        messages(&sent),
        vec![
            SystemMessageId::TheSoulshotYouAreAttemptingToUseDoesNotMatchTheGradeOfYourEquippedWeapon,
            SystemMessageId::TheAutomaticUseOfS1HasBeenActivated,
        ],
        "{:?}",
        names(&sent)
    );
    assert_eq!(auto_shot_packets(&sent), vec![(SOULSHOT_D, 1)]);
    assert_eq!(s.count_of(SOULSHOT_D), 10);
}

/// Not enough shots: the message, or, for an automatic shot, it is switched off with
/// `ExAutoSoulShot(.., 0)` and "The automatic use of $s1 has been deactivated.".
#[test]
fn not_enough_shots_message_or_automatic_use_ends() {
    let mut s = Scene::new(vec![worn(SWORD_NG), bag(SOULSHOT_NG, 1)]);
    let sent = s.use_item(SHOTS_OBJ);
    assert_eq!(
        messages(&sent),
        vec![SystemMessageId::YouDoNotHaveEnoughSoulshotsForThat]
    );
    assert_eq!(s.count_of(SOULSHOT_NG), 1);
    assert!(!s.charged(ShotType::Soulshots));

    // Automatic: the request itself tries once and fails, ending the automatic use again.
    let sent = s.auto(SOULSHOT_NG, true);
    assert_eq!(
        messages(&sent),
        vec![
            SystemMessageId::TheAutomaticUseOfS1HasBeenActivated,
            SystemMessageId::TheAutomaticUseOfS1HasBeenDeactivated,
        ]
    );
    assert_eq!(auto_shot_packets(&sent), vec![(SOULSHOT_NG, 1), (SOULSHOT_NG, 0)]);
    assert!(!s.world.has_auto_soul_shot(ME, SOULSHOT_NG));
}

/// Spirit and blessed spirit shots use the weapon's spiritshot count and are charged
/// independently ("it can be charged over SpiritShot").
#[test]
fn spiritshots_and_blessed_spiritshots_charge_separately() {
    let mut s = Scene::new(vec![
        worn(SWORD_NG),
        bag(SPIRITSHOT_NG, 10),
        item(ObjectId(0x3000_0003), BLESSED_NG, "INVENTORY", 1, 10),
    ]);
    let sent = s.use_item(SHOTS_OBJ);
    assert_eq!(
        messages(&sent),
        vec![SystemMessageId::YourSpiritshotHasBeenEnabled]
    );
    assert_eq!(s.count_of(SPIRITSHOT_NG), 8);
    assert!(s.charged(ShotType::Spiritshots));
    assert!(!s.charged(ShotType::BlessedSpiritshots));
    let animation = decode::<MagicSkillUse>(of(&sent, "MAGIC_SKILL_USE")[0]);
    assert_eq!(animation.skill_id, 2061);

    let sent = s.use_item(ObjectId(0x3000_0003));
    assert_eq!(
        messages(&sent),
        vec![SystemMessageId::YourSpiritshotHasBeenEnabled]
    );
    assert_eq!(s.count_of(BLESSED_NG), 8);
    assert!(s.charged(ShotType::Spiritshots) && s.charged(ShotType::BlessedSpiritshots));

    // Both charged: nothing more.
    assert!(messages(&s.use_item(SHOTS_OBJ)).is_empty());
    assert!(messages(&s.use_item(ObjectId(0x3000_0003))).is_empty());
    assert_eq!(s.count_of(SPIRITSHOT_NG) + s.count_of(BLESSED_NG), 16);
}

/// A weapon that has a reduced soulshot count (`reduced_soulshot` "chance,count") uses that many
/// when the roll passes (`Rnd.get(100) < chance`; here always), and the grade matches by the
/// weapon's crystal type.
#[test]
fn reduced_soulshot_count_and_matching_grade() {
    let mut s = Scene::new(vec![worn(SWORD_D), bag(SOULSHOT_D, 10)]);
    let sent = s.use_item(SHOTS_OBJ);
    assert_eq!(messages(&sent), vec![SystemMessageId::YourSoulshotsAreEnabled]);
    assert_eq!(
        s.count_of(SOULSHOT_D),
        9,
        "the reduced count of 1, not the 3 of the weapon"
    );
    let animation = decode::<MagicSkillUse>(of(&sent, "MAGIC_SKILL_USE")[0]);
    assert_eq!(animation.skill_id, 2150);
}

/// The whole stack can go: using exactly what is there works, the item is gone afterwards.
#[test]
fn the_last_shots_of_a_stack_are_used() {
    let mut s = Scene::new(vec![worn(SWORD_NG), bag(SOULSHOT_NG, 2)]);
    s.use_item(SHOTS_OBJ);
    assert_eq!(s.count_of(SOULSHOT_NG), 0);
    assert!(s.charged(ShotType::Soulshots));
}

// ---- automatic use -----------------------------------------------------------------------------

/// `RequestAutoSoulShot` on: `ExAutoSoulShot(id, 1)`, the activation message, and a first shot
/// at once (`rechargeShots`); off: `ExAutoSoulShot(id, 0)` and the deactivation message. A wrong
/// type value does nothing.
#[test]
fn auto_shots_on_and_off() {
    let mut s = Scene::new(vec![worn(SWORD_NG), bag(SOULSHOT_NG, 10)]);
    let sent = s.auto(SOULSHOT_NG, true);
    assert_eq!(auto_shot_packets(&sent), vec![(SOULSHOT_NG, 1)]);
    assert_eq!(
        messages(&sent),
        vec![
            SystemMessageId::TheAutomaticUseOfS1HasBeenActivated,
            SystemMessageId::YourSoulshotsAreEnabled,
        ]
    );
    let activated =
        SystemMessage::decode(&mut PacketReader::new(&of(&sent, "SYSTEM_MESSAGE")[0][1..])).unwrap();
    assert_eq!(activated.params, vec![SystemMessageParam::ItemName(SOULSHOT_NG)]);
    assert!(s.world.has_auto_soul_shot(ME, SOULSHOT_NG));
    assert!(s.charged(ShotType::Soulshots));
    assert_eq!(s.count_of(SOULSHOT_NG), 8);

    s.input(ClientInput::AutoSoulShot {
        item_id: SOULSHOT_NG,
        shot_type: 7,
    });
    assert!(s.log.take().is_empty(), "Java acts on 0 and 1 only");

    let sent = s.auto(SOULSHOT_NG, false);
    assert_eq!(auto_shot_packets(&sent), vec![(SOULSHOT_NG, 0)]);
    assert_eq!(
        messages(&sent),
        vec![SystemMessageId::TheAutomaticUseOfS1HasBeenDeactivated]
    );
    assert!(!s.world.has_auto_soul_shot(ME, SOULSHOT_NG));

    // An item the player does not have: ignored.
    assert!(s.auto(2514, true).is_empty());
}

/// Fishing shots are not automatic and beast shots need a summon.
#[test]
fn fishing_and_beast_shots_cannot_be_automatic() {
    let mut s = Scene::new(vec![
        worn(SWORD_NG),
        bag(6535, 10),
        item(ObjectId(0x3000_0003), 6645, "INVENTORY", 1, 10),
    ]);
    assert!(s.auto(6535, true).is_empty());
    let sent = s.auto(6645, true);
    assert_eq!(
        messages(&sent),
        vec![SystemMessageId::YouDoNotHaveAServitorOrPetAndThereforeCannotUseTheAutomaticUseFunction]
    );
    assert!(!s.world.has_auto_soul_shot(ME, 6645));
}

/// Taking a weapon on charges the automatic shots (`Player.useEquippableItem` ->
/// `rechargeShots(true, true)` for a right hand weapon).
#[test]
fn equipping_a_weapon_recharges_automatic_shots() {
    let mut s = Scene::new(vec![
        item(SWORD_OBJ, SWORD_NG, "INVENTORY", 0, 1),
        bag(SOULSHOT_NG, 10),
    ]);
    s.auto(SOULSHOT_NG, true);
    assert!(
        !s.charged(ShotType::Soulshots),
        "no weapon yet: the request only switched it on"
    );
    assert_eq!(s.count_of(SOULSHOT_NG), 10);
    s.use_item(SWORD_OBJ);
    assert!(s.charged(ShotType::Soulshots));
    assert_eq!(s.count_of(SOULSHOT_NG), 8);
}

/// A shortcut to the shots that goes with the item (destroyed here) ends the automatic use: the
/// client gets `ExAutoSoulShot(id, 0)` before the new shortcut list (`Shortcuts.deleteShortcut`).
#[test]
fn destroying_the_item_of_a_shortcut_ends_the_automatic_use() {
    let shortcut = ShortcutData {
        slot: 0,
        page: 0,
        kind: 1,
        shortcut_id: SHOTS_OBJ.0,
        level: 0,
    };
    let mut s = Scene::with_shortcuts(vec![worn(SWORD_NG), bag(SOULSHOT_NG, 10)], vec![shortcut]);
    s.auto(SOULSHOT_NG, true);
    s.auto(SOULSHOT_NG, false);
    s.auto(SOULSHOT_NG, true);
    s.input(ClientInput::DestroyItem {
        object_id: SHOTS_OBJ.0,
        count: 10,
    });
    let sent = s.log.take();
    let order = names(&sent);
    let off = order.iter().position(|n| n == "EX_AUTO_SOULSHOT").unwrap();
    let init = order.iter().position(|n| n == "SHORT_CUT_INIT").unwrap();
    assert!(off < init, "{order:?}");
    assert_eq!(auto_shot_packets(&sent), vec![(SOULSHOT_NG, 0)]);
    assert!(!s.world.has_auto_soul_shot(ME, SOULSHOT_NG));
}

// ---- attacks -----------------------------------------------------------------------------------

/// Attacks `monster` once with the player (the attack packet is the player's first `Attack`).
fn one_attack(s: &mut Scene, monster: ObjectId) -> Attack {
    s.world.do_attack(ME, monster);
    let sent = s.log.take();
    let attack = decode::<Attack>(of(&sent, "ATTACK")[0]);
    assert_eq!(attack.attacker_id, ME.0);
    attack
}

/// A soulshot-charged hit has the `USESS` flag (0x10) with the weapon's grade bits, does twice the
/// damage of the same hit without one (same seed, same rolls), and discharges the weapon.
#[test]
fn a_charged_attack_has_the_soulshot_flag_doubles_damage_and_discharges() {
    let mut plain = Scene::new(vec![worn(SWORD_NG)]);
    let monster = plain.world.spawn_npc_at(9001, X + 40, Y, Z, 0).unwrap();
    plain.log.take();
    let without = one_attack(&mut plain, monster);

    let mut charged = Scene::new(vec![worn(SWORD_NG), bag(SOULSHOT_NG, 10)]);
    let monster = charged.world.spawn_npc_at(9001, X + 40, Y, Z, 0).unwrap();
    charged.use_item(SHOTS_OBJ);
    assert!(charged.charged(ShotType::Soulshots));
    charged.log.take();
    let with = one_attack(&mut charged, monster);

    assert_eq!(without.first_hit.flags & 0x10, 0);
    assert_eq!(with.first_hit.flags & 0x10, 0x10);
    // Same rolls: the miss and critical bits agree.
    assert_eq!(without.first_hit.flags & 0xE0, with.first_hit.flags & 0xE0);
    if with.first_hit.flags & 0x80 == 0 {
        // `ss ? 2 : 1` multiplies the attack before the defence: about twice the damage.
        let (a, b) = (
            f64::from(without.first_hit.damage),
            f64::from(with.first_hit.damage),
        );
        assert!(b > a * 1.8 && b < a * 2.2, "{a} vs {b}");
    }
    assert!(
        !charged.charged(ShotType::Soulshots),
        "a hit that was not a miss in the attack used the charge up (`hitted`)"
    );
}

/// An automatic soulshot is charged again after the hit lands (`HitTask` -> `rechargeShots`), so
/// the next attack is charged too; the shots are consumed once per charge.
#[test]
fn automatic_soulshots_are_recharged_after_each_hit() {
    let mut s = Scene::new(vec![worn(SWORD_NG), bag(SOULSHOT_NG, 20)]);
    let monster = s.world.spawn_npc_at(9001, X + 40, Y, Z, 0).unwrap();
    s.auto(SOULSHOT_NG, true);
    assert_eq!(s.count_of(SOULSHOT_NG), 18);
    let first = one_attack(&mut s, monster);
    assert_eq!(first.first_hit.flags & 0x10, 0x10);
    assert!(!s.charged(ShotType::Soulshots), "used by the attack");
    // The hit lands half an attack later and charges the next shot.
    s.ticks(20);
    assert!(s.charged(ShotType::Soulshots));
    assert_eq!(s.count_of(SOULSHOT_NG), 16);
    // Let the attack animation end, then hit again.
    s.ticks(60);
    s.log.take();
    let second = one_attack(&mut s, monster);
    assert_eq!(second.first_hit.flags & 0x10, 0x10);
}

/// A monster that has soulshots in its template (`<shots soul="3" shotChance="100">`) charges one
/// when its first hit lands (`Npc.rechargeShots`, with the `MagicSkillUse` of skill 2154) and
/// the next attack carries the flag; it has 3 in all.
#[test]
fn monsters_use_the_soulshots_of_their_template() {
    let mut s = Scene::new(vec![]);
    let npc = s.world.spawn_npc_at(9002, X + 30, Y, Z, 0).unwrap();
    s.log.take();
    assert!(!s.world.is_charged_shot(npc, ShotType::Soulshots));
    s.world.do_attack(npc, ME);
    let first = decode::<Attack>(of(&s.log.take(), "ATTACK")[0]);
    assert_eq!(first.first_hit.flags & 0x10, 0, "nothing charged yet");
    // The hit lands, then the monster loads its first soulshot.
    s.ticks(30);
    assert!(s.world.is_charged_shot(npc, ShotType::Soulshots));
    let sent = s.log.take();
    let animation = decode::<MagicSkillUse>(of(&sent, "MAGIC_SKILL_USE")[0]);
    assert_eq!(
        (animation.creature_id, animation.target_id, animation.skill_id),
        (npc.0, npc.0, 2154)
    );
    s.ticks(80);
    s.log.take();
    s.world.do_attack(npc, ME);
    let second = decode::<Attack>(of(&s.log.take(), "ATTACK")[0]);
    assert_eq!(second.first_hit.flags & 0x10, 0x10);
    assert!(!s.world.is_charged_shot(npc, ShotType::Soulshots));
}

// ---- skills ------------------------------------------------------------------------------------

/// `Creature.beginCast`: a magic skill with spiritshots (or blessed ones) charged is cast in 60 %
/// of the time (`(int) (0.6 * skillTime)`); a physical one is not affected.
#[test]
fn spiritshots_cut_the_cast_time_of_magic_skills_to_60_percent() {
    let mut s = Scene::new(vec![
        worn(SWORD_NG),
        bag(SPIRITSHOT_NG, 10),
        item(ObjectId(0x3000_0003), BLESSED_NG, "INVENTORY", 1, 10),
    ]);
    let skills = stat_data().skills;
    let wind_strike = skills.get(1177, 1).unwrap();
    let power_strike = skills.get(9006, 1).unwrap();
    let plain = s.world.skill_cast_time(ME, wind_strike);
    let physical = s.world.skill_cast_time(ME, power_strike);
    assert!(plain > 550);
    s.use_item(SHOTS_OBJ);
    assert_eq!(
        s.world.skill_cast_time(ME, wind_strike),
        (0.6 * f64::from(plain)) as i32
    );
    assert_eq!(s.world.skill_cast_time(ME, power_strike), physical);
    // Blessed alone does the same.
    s.world.set_charged_shot(ME, ShotType::Spiritshots, false);
    assert_eq!(s.world.skill_cast_time(ME, wind_strike), plain);
    s.use_item(ObjectId(0x3000_0003));
    assert_eq!(
        s.world.skill_cast_time(ME, wind_strike),
        (0.6 * f64::from(plain)) as i32
    );
}

/// A magic skill uses the spiritshot charge up when it has landed (`Skill.activateSkill`). Without
/// automatic use the weapon stays empty afterwards.
#[test]
fn a_cast_uses_the_charge_up() {
    let mut s = Scene::new(vec![worn(STAFF_NG), bag(SPIRITSHOT_NG, 10)]);
    s.use_item(SHOTS_OBJ);
    assert!(s.charged(ShotType::Spiritshots));
    assert_eq!(s.count_of(SPIRITSHOT_NG), 9, "the staff uses one");
    s.input(ClientInput::MagicSkillUse {
        magic_id: 9002,
        ctrl_pressed: false,
        shift_pressed: false,
    });
    s.ticks(100);
    assert!(!s.charged(ShotType::Spiritshots), "the cast used the charge up");
    assert_eq!(s.count_of(SPIRITSHOT_NG), 9);
}

/// With automatic spiritshots the hit of the cast already finds a charge (nothing more is
/// consumed there), the skill uses it up, and the finalizer of the cast loads the next one
/// (`onMagicFinalizer`: `mut.getCount() > 0` is true after any cast with a cast time). An empty
/// weapon is loaded at the hit.
#[test]
fn automatic_spiritshots_are_recharged_by_the_cast() {
    let mut s = Scene::new(vec![worn(STAFF_NG), bag(SPIRITSHOT_NG, 10)]);
    s.auto(SPIRITSHOT_NG, true);
    assert!(s.charged(ShotType::Spiritshots));
    assert_eq!(s.count_of(SPIRITSHOT_NG), 9);
    s.input(ClientInput::MagicSkillUse {
        magic_id: 9002,
        ctrl_pressed: false,
        shift_pressed: false,
    });
    s.ticks(100);
    assert!(s.charged(ShotType::Spiritshots), "loaded again by the finalizer");
    assert_eq!(
        s.count_of(SPIRITSHOT_NG),
        8,
        "one used up by the skill, one loaded"
    );

    // Taken off the weapon by hand: the hit of the next cast loads one before the skill lands.
    s.world.set_charged_shot(ME, ShotType::Spiritshots, false);
    s.input(ClientInput::MagicSkillUse {
        magic_id: 9002,
        ctrl_pressed: false,
        shift_pressed: false,
    });
    s.ticks(100);
    assert!(s.charged(ShotType::Spiritshots));
    assert_eq!(
        s.count_of(SPIRITSHOT_NG),
        6,
        "one at the hit, one by the finalizer"
    );
}

/// `Skill.useSoulShot()` / `useSpiritShot()`: physical attack effects use soulshots, magic skills
/// spiritshots, a buff neither.
#[test]
fn which_skills_use_which_shots() {
    let mut s = Scene::new(vec![worn(SWORD_NG)]);
    let skills = stat_data().skills;
    let wind_strike = skills.get(1177, 1).unwrap();
    let power_strike = skills.get(9006, 1).unwrap();
    let buff = skills.get(9007, 1).unwrap();
    assert!(s.world.skill_uses_spirit_shot(wind_strike));
    assert!(!s.world.skill_uses_soul_shot(wind_strike));
    assert!(s.world.skill_uses_soul_shot(power_strike));
    assert!(!s.world.skill_uses_spirit_shot(power_strike));
    assert!(!s.world.skill_uses_soul_shot(buff) && !s.world.skill_uses_spirit_shot(buff));
}

/// The charges are on the weapon: they are gone with the player (a new login has none), and
/// `un_charge`ing other creatures does nothing.
#[test]
fn charges_are_not_kept_across_a_login() {
    let mut s = Scene::new(vec![worn(SWORD_NG), bag(SOULSHOT_NG, 10)]);
    s.use_item(SHOTS_OBJ);
    assert!(s.charged(ShotType::Soulshots));
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    let mut again = Scene::new(vec![worn(SWORD_NG), bag(SOULSHOT_NG, 10)]);
    assert!(!again.charged(ShotType::Soulshots));
    assert!(!again.world.is_charged_shot(ObjectId(1), ShotType::Soulshots));
    again
        .world
        .set_charged_shot(ObjectId(1), ShotType::Soulshots, true);
    assert!(!again.world.is_charged_shot(ObjectId(1), ShotType::Soulshots));
}
