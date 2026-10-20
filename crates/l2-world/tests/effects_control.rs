#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The status effect handlers (M5-EFX-02), through the world with recording clients: for each
//! handler that it lands, what the world shows of it (flags, abnormal visual effects, packets),
//! what it blocks and that it ends cleanly. The skills are synthetic (one per handler, named
//! after the Java class); the expected behaviour comes from the handler classes in
//! `dist/game/data/scripts/handlers/skill/effects/` and the checks in `Creature`, `CreatureAI`,
//! `AttackableAI`, `Player` and the client packets named in each test.

use std::sync::{Arc, Mutex};

use l2_data::skill::{AbnormalVisualEffect, TraitType};
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{
    ChangeWaitType, CharInfo, EtcStatusUpdate, FlyToLocation, StartRotation, StopRotation, SystemMessage,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::*;

// ---- harness ---------------------------------------------------------------------------------------

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
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn count(sent: &Sent, to: i32, name: &str) -> usize {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .count()
}

fn of<'a>(sent: &'a Sent, to: i32, name: &str) -> Vec<&'a [u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .map(|(_, b)| b.as_slice())
        .collect()
}

fn message_ids(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    of(sent, to, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p).id)
        .collect()
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
</staticData><lvlUpgainData><level val="1"><hp>5000</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
</list>"#;

const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="9002" type="Monster" level="1" name="Hare">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300" isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9001" type="Monster" level="1" name="Wolf">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
</list>"#;

/// One synthetic skill: `kind` is the operate type (`A2` continuous, `A1` instant, `T` toggle,
/// `P` passive), `abnormal` its abnormal type, `visual` its abnormal visual effect.
fn skill(id: i32, kind: &str, abnormal: &str, visual: &str, effects: &str) -> String {
    let abnormal = if abnormal.is_empty() {
        String::new()
    } else {
        format!(
            "<abnormalTime>60</abnormalTime><abnormalType>{abnormal}</abnormalType><abnormalLevel>1</abnormalLevel>"
        )
    };
    let visual = if visual.is_empty() {
        String::new()
    } else {
        format!("<abnormalVisualEffect>{visual}</abnormalVisualEffect>")
    };
    // A negative effect point makes it a harmful skill (`Skill.hasNegativeEffect()`).
    format!(
        r#"<skill id="{id}" levels="1" name="S{id}"><operateType>{kind}</operateType><targetType>ONE</targetType><effectPoint>-100</effectPoint>{abnormal}{visual}<effects>{effects}</effects></skill>"#
    )
}

fn skills_xml() -> String {
    let list = [
        skill(9101, "A2", "STUN", "STUN", r#"<effect name="Stun"/>"#),
        skill(9102, "A2", "SLEEP", "SLEEP", r#"<effect name="Sleep"/>"#),
        skill(9103, "A2", "ROOT_MAGICALLY", "ROOT", r#"<effect name="Root"/>"#),
        skill(9104, "A2", "PARALYZE", "PARALYZE", r#"<effect name="Paralyze"/>"#),
        skill(9105, "A2", "TURN_STONE", "", r#"<effect name="Petrification"/>"#),
        skill(9106, "A2", "MULTI_DEBUFF", "", r#"<effect name="Fear"/>"#),
        skill(9107, "A2", "SILENCE", "", r#"<effect name="Mute"/>"#),
        skill(
            9108,
            "A2",
            "SILENCE_PHYSICAL",
            "",
            r#"<effect name="PhysicalMute"/>"#,
        ),
        skill(
            9109,
            "A2",
            "SILENCE_ALL",
            "",
            r#"<effect name="PhysicalDamageMute"/>"#,
        ),
        skill(9110, "A2", "DISARM", "", r#"<effect name="Disarm"/>"#),
        skill(9111, "A2", "INVINCIBILITY", "", r#"<effect name="Invincible"/>"#),
        skill(9112, "A2", "HIDE", "", r#"<effect name="Hide"/>"#),
        skill(
            9113,
            "T",
            "",
            "",
            r#"<effect name="SilentMove" ticks="1"><power>10</power></effect>"#,
        ),
        skill(
            9114,
            "T",
            "",
            "",
            r#"<effect name="FakeDeath" ticks="1"><power>0</power></effect>"#,
        ),
        skill(
            9115,
            "T",
            "",
            "",
            r#"<effect name="Relax" ticks="1"><power>0</power></effect>"#,
        ),
        skill(
            9116,
            "A2",
            "MULTI_BUFF",
            "",
            r#"<effect name="BlockAbnormalSlot"><slot>PD_UP;MD_UP</slot></effect>"#,
        ),
        skill(
            9117,
            "A2",
            "MULTI_BUFF_A",
            "",
            r#"<effect name="EnlargeAbnormalSlot"><slots>4</slots></effect>"#,
        ),
        skill(
            9118,
            "A2",
            "MULTI_DEBUFF_A",
            "",
            r#"<effect name="ResistSkill"><skillId1>9101</skillId1><skillLevel1>1</skillLevel1></effect>"#,
        ),
        skill(
            9119,
            "A2",
            "MULTI_DEBUFF_B",
            "",
            r#"<effect name="AttackTrait"><SHOCK>50</SHOCK></effect>"#,
        ),
        skill(
            9120,
            "A2",
            "MULTI_DEBUFF_C",
            "",
            r#"<effect name="DefenceTrait"><SLEEP>100</SLEEP><HOLD>20</HOLD></effect>"#,
        ),
        skill(9121, "A2", "MULTI_DEBUFF_D", "", r#"<effect name="Flag"/>"#),
        skill(
            9122,
            "A1",
            "",
            "",
            r#"<effect name="TargetCancel"><chance>100</chance></effect>"#,
        ),
        skill(
            9123,
            "A1",
            "",
            "",
            r#"<effect name="Bluff"><chance>100</chance></effect>"#,
        ),
        skill(
            9124,
            "A1",
            "",
            "",
            r#"<effect name="SkillTurning"><chance>100</chance></effect>"#,
        ),
        skill(9125, "A1", "", "", r#"<effect name="EnemyCharge"/>"#),
        skill(
            9126,
            "A1",
            "",
            "",
            r#"<effect name="FocusEnergy"><charge>2</charge></effect>"#,
        ),
        skill(
            9129,
            "A2",
            "MULTI_DEBUFF_E",
            "",
            r#"<effect name="SoulEating"><expNeeded>5</expNeeded></effect>"#,
        ),
        skill(9130, "A2", "DISTRUST", "", r#"<effect name="Distrust"/>"#),
        skill(
            9131,
            "A1",
            "",
            "",
            r#"<effect name="RunAway"><power>100</power><time>10</time></effect>"#,
        ),
        skill(
            9132,
            "A1",
            "",
            "",
            r#"<effect name="Confuse"><chance>100</chance></effect>"#,
        ),
        skill(9133, "A2", "MULTI_DEBUFF_F", "", r#"<effect name="BlockChat"/>"#),
        skill(
            9134,
            "A2",
            "BOT_PENALTY",
            "",
            r#"<effect name="BlockAction"><blockedActions>-1,-3,-5</blockedActions></effect>"#,
        ),
        skill(9135, "A2", "HP_REGEN_UP", "", r#"<effect name="ImmobileBuff"/>"#),
        skill(9136, "A2", "TURN_PASSIVE", "", r#"<effect name="Passive"/>"#),
        skill(
            9137,
            "A2",
            "PUBLIC_SLOT",
            "",
            r#"<effect name="ProtectionBlessing"/>"#,
        ),
        skill(9138, "A2", "PROTECTION", "", r#"<effect name="NoblesseBless"/>"#),
        skill(
            9139,
            "A2",
            "BLOCK_RESURRECTION",
            "",
            r#"<effect name="BlockResurrection"/>"#,
        ),
        skill(
            9140,
            "T",
            "",
            "",
            r#"<effect name="ChameleonRest" ticks="1"><power>0</power></effect>"#,
        ),
        skill(9141, "A2", "MULTI_DEBUFF_G", "", r#"<effect name="Debuff"/>"#),
        skill(9142, "A2", "MULTI_BUFF_A", "", r#"<effect name="Betray"/>"#),
        skill(
            9143,
            "P",
            "",
            "",
            r#"<effect name="TalismanSlot"><slots>3</slots></effect>"#,
        ),
        skill(9144, "P", "", "", r#"<effect name="EnableCloak"/>"#),
        skill(
            9145,
            "P",
            "",
            "",
            r#"<effect name="CrystalGradeModify"><grade>2</grade></effect>"#,
        ),
        skill(
            9146,
            "A2",
            "MULTI_DEBUFF_SOUL",
            "",
            r#"<effect name="PolearmSingleTarget"/>"#,
        ),
        skill(9147, "A2", "POISON", "", r#"<effect name="Lucky"/>"#),
        skill(
            9148,
            "A2",
            "MULTI_BUFF",
            "",
            r#"<effect name="BlockAbnormalSlot"><slot>NO_SUCH_SLOT</slot></effect>"#,
        ),
        skill(9149, "A2", "PD_UP", "", r#"<effect name="Buff"/>"#),
        skill(
            9150,
            "T",
            "",
            "",
            r#"<effect name="SilentMove" ticks="1"><power>0</power></effect>"#,
        ),
    ];
    format!("<list>{}</list>", list.concat())
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const OTHER: ObjectId = ObjectId(0x2000_0002);

fn player_data(id: ObjectId, name: &str, (x, y, z): (i32, i32, i32), items: Vec<ItemData>) -> PlayerData {
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
        level: 1,
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
        cur_hp: 5000.0,
        max_hp: 5000.0,
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

fn sword() -> Vec<ItemData> {
    vec![ItemData {
        object_id: ObjectId(0x3000_0001),
        item_id: 2,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: i32::try_from(paperdoll::RHAND).unwrap(),
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }]
}

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new() -> Self {
        Self::with(EffectSettings::default())
    }

    fn with(effects: EffectSettings) -> Self {
        let log = Log::default();
        let stats = StatData {
            items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
            player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::from_xml(
                &skills_xml(),
                &l2_data::skill::EnchantSkillGroups::default(),
                &l2_data::SkillLoadOptions::default(),
            )),
        };
        let mut world = World::new(WorldRng::from_seed(20_261_020), Box::new(NoOutbox));
        world.set_settings(WorldSettings {
            effects,
            // General chat from level 1, so the chat tests need no level.
            minimum_chat_level: 0,
            ..WorldSettings::default()
        });
        world.set_static_data(Box::new(Data(stats)));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        let mut scene = Self { world, log };
        scene.enter(ME, "Me", (X, Y, Z), sword());
        scene.enter(OTHER, "Other", (X + 50, Y, Z), vec![]);
        scene.run(1000);
        scene.log.take();
        scene
    }

    fn enter(&mut self, id: ObjectId, name: &str, pos: (i32, i32, i32), items: Vec<ItemData>) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id.0).unwrap()),
            player: Box::new(player_data(id, name, pos, items)),
            sink: Box::new(Sink(id.0, self.log.clone())),
            ip: None,
        });
    }

    /// Runs the world for `ms` milliseconds (50 ms ticks) and returns what was sent meanwhile.
    fn run(&mut self, ms: u64) -> Sent {
        let mut sent = Vec::new();
        for _ in 0..(ms / 50) {
            self.world.tick();
            sent.extend(self.log.take());
        }
        sent
    }

    fn hare(&mut self, dx: i32) -> ObjectId {
        let id = self.world.spawn_npc_at(9002, X + dx, Y + 50, Z, 0).unwrap();
        self.run(100);
        id
    }

    fn apply(&mut self, from: ObjectId, to: ObjectId, skill: i32) {
        self.world.apply_effects(from, to, SkillKey::new(skill, 1));
    }

    fn visual(&self, id: ObjectId) -> i32 {
        self.world.abnormal_visual_effects(id).0
    }

    fn input(&mut self, id: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id.0).unwrap()),
            input,
        });
    }
}

fn mask(e: AbnormalVisualEffect) -> i32 {
    e.mask()
}

fn say(s: &mut Scene, text: &str) {
    s.input(
        ME,
        ClientInput::Say {
            chat_type: 0,
            text: text.into(),
            target: None,
        },
    );
}

// ---- the registry ----------------------------------------------------------------------------------

#[test]
fn every_handler_of_the_card_is_ported() {
    let registry = l2_world::effects::EffectRegistry::new();
    for name in [
        "Debuff",
        "ImmobileBuff",
        "Stun",
        "Sleep",
        "Root",
        "Paralyze",
        "Petrification",
        "Fear",
        "Confuse",
        "Betray",
        "Mute",
        "PhysicalMute",
        "PhysicalDamageMute",
        "Disarm",
        "Invincible",
        "Hide",
        "SilentMove",
        "FakeDeath",
        "Relax",
        "ChameleonRest",
        "BlockAction",
        "BlockChat",
        "BlockResurrection",
        "BlockAbnormalSlot",
        "EnlargeAbnormalSlot",
        "ResistSkill",
        "AttackTrait",
        "DefenceTrait",
        "Lucky",
        "NoblesseBless",
        "ProtectionBlessing",
        "Passive",
        "Flag",
        "TargetCancel",
        "Bluff",
        "Distrust",
        "RunAway",
        "SkillTurning",
        "EnemyCharge",
        "FocusEnergy",
        "FocusMaxEnergy",
        "FocusSouls",
        "SoulEating",
        "TalismanSlot",
        "EnableCloak",
        "CrystalGradeModify",
        "PolearmSingleTarget",
    ] {
        assert!(registry.is_ported(name), "{name} is not ported");
    }
}

// ---- stun, sleep, root, paralysis, petrification --------------------------------------------------

/// `Stun`: `isStunned()` makes the creature `isDisabled()`, `isAllSkillsDisabled()` and
/// `isMovementDisabled()`; `startStunning` stops its attack and movement and shows the effect.
#[test]
fn stun_holds_a_player_and_ends_cleanly() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(hare, ME, 9101);
    assert!(s.world.is_stunned(ME));
    assert!(s.world.is_disabled(ME) && s.world.is_all_skills_disabled(ME));
    assert!(s.world.is_attack_disabled(ME));
    // The abnormal visual effect is part of the player's own info and what others see.
    assert_ne!(s.visual(ME) & mask(AbnormalVisualEffect::Stun), 0);
    s.log.take();
    s.world.handle(WorldCommand::EnterWorld {
        session: SessionId(9),
        player: Box::new(player_data(ObjectId(0x2000_0009), "Late", (X + 20, Y, Z), vec![])),
        sink: Box::new(Sink(0x2000_0009, s.log.clone())),
        ip: None,
    });
    let info: CharInfo = decode(of(&s.log.take(), 0x2000_0009, "CHAR_INFO").first().unwrap());
    assert_ne!(info.abnormal_visual_effects & mask(AbnormalVisualEffect::Stun), 0);

    // A stunned player cannot attack or walk.
    let victim = s.hare(0);
    s.log.take();
    s.world.do_attack(ME, victim);
    s.input(
        ME,
        ClientInput::MoveTo {
            target_x: X + 300,
            target_y: Y,
            target_z: Z,
            origin_x: X,
            origin_y: Y,
            origin_z: Z,
            movement_mode: 1,
        },
    );
    let sent = s.run(500);
    assert_eq!(count(&sent, ME.0, "ATTACK"), 0);
    assert_eq!(count(&sent, ME.0, "MOVE_TO_LOCATION"), 0);

    // 60 s later it wears off: free again, the visual effect is gone.
    s.run(60_100);
    assert!(!s.world.is_stunned(ME) && !s.world.is_movement_disabled(ME));
    assert_eq!(s.visual(ME) & mask(AbnormalVisualEffect::Stun), 0);
}

#[test]
fn a_stunned_monster_stops_and_does_not_think() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    // The hare fights back once hit; a stun stops that.
    s.world.set_intention_attack(hare, ME);
    s.run(200);
    s.apply(ME, hare, 9101);
    assert!(s.world.is_stunned(hare) && s.world.is_all_skills_disabled(hare));
    // `startStunning`: `setIntentionIdle()`, which an attackable with a player in view turns
    // into ACTIVE again (`shouldPromoteIdleToActive`); it is not fighting.
    assert_ne!(s.world.intention_of(hare), Intention::Attack);
    assert!(!s.world.is_auto_attacking(hare));
    // The NPC info of the others shows the effect at once.
    assert_ne!(s.visual(hare) & mask(AbnormalVisualEffect::Stun), 0);
    s.run(60_100);
    assert!(!s.world.is_stunned(hare));
}

#[test]
fn damage_may_break_a_stun() {
    // `PlayerStatus.reduceHp`: a stunned player has a one in ten chance per hit to be freed.
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(hare, ME, 9101);
    for _ in 0..200 {
        s.world.reduce_current_hp(ME, 1.0, Some(hare));
        if !s.world.is_stunned(ME) {
            break;
        }
    }
    assert!(!s.world.is_stunned(ME), "200 hits never broke the stun");
}

#[test]
fn sleep_blocks_like_a_stun_and_wakes_the_ai() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(ME, hare, 9102);
    assert!(s.world.is_sleeping(hare) && s.world.is_all_skills_disabled(hare));
    assert!(s.world.is_movement_disabled(hare) && s.world.is_disabled(hare));
    assert_ne!(s.visual(hare) & mask(AbnormalVisualEffect::Sleep), 0);
    s.run(60_100);
    assert!(!s.world.is_sleeping(hare));
    assert_eq!(s.visual(hare) & mask(AbnormalVisualEffect::Sleep), 0);
}

#[test]
fn root_blocks_movement_but_not_attacks() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(hare, ME, 9103);
    assert!(s.world.is_rooted(ME) && s.world.is_movement_disabled(ME));
    assert!(!s.world.is_disabled(ME) && !s.world.is_all_skills_disabled(ME));
    s.log.take();
    s.input(
        ME,
        ClientInput::MoveTo {
            target_x: X + 300,
            target_y: Y,
            target_z: Z,
            origin_x: X,
            origin_y: Y,
            origin_z: Z,
            movement_mode: 1,
        },
    );
    assert_eq!(count(&s.run(300), ME.0, "MOVE_TO_LOCATION"), 0);
    s.run(60_100);
    assert!(!s.world.is_rooted(ME) && !s.world.is_movement_disabled(ME));
}

#[test]
fn paralysis_holds_and_petrification_also_makes_invulnerable() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(hare, ME, 9104);
    assert!(s.world.is_paralyzed(ME) && s.world.is_movement_disabled(ME) && s.world.is_disabled(ME));
    assert!(!s.world.is_invul(ME));
    s.run(60_100);
    assert!(!s.world.is_paralyzed(ME));

    s.apply(hare, ME, 9105);
    assert!(s.world.is_paralyzed(ME) && s.world.is_invul(ME));
    // Invulnerable: a hit does nothing.
    s.world.reduce_current_hp(ME, 100.0, Some(hare));
    assert_eq!(s.world.player(ME).unwrap().core.cur_hp, 5000.0);
    s.run(60_100);
    assert!(!s.world.is_paralyzed(ME) && !s.world.is_invul(ME));
}

// ---- fear, confusion, betrayal, runaway ------------------------------------------------------------

#[test]
fn fear_sends_a_player_running_and_ticks_every_five_ticks() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.log.take();
    s.apply(hare, ME, 9106);
    assert!(s.world.is_control_blocked(ME) && s.world.is_out_of_control(ME));
    // `notifyActionAfraid(effector, true)`: a move away from the effector.
    let first = s.run(100);
    assert!(count(&first, ME.0, "MOVE_TO_LOCATION") >= 1);
    // `getTicks()` is 5 whatever the skill says: the effect ticks every 5 * 666 ms and moves the
    // player again.
    let later = s.run(4_000);
    assert!(count(&later, ME.0, "MOVE_TO_LOCATION") >= 1);
    // A feared player cannot be ordered about, nor attack.
    s.log.take();
    s.input(
        ME,
        ClientInput::MoveTo {
            target_x: X - 300,
            target_y: Y,
            target_z: Z,
            origin_x: X,
            origin_y: Y,
            origin_z: Z,
            movement_mode: 1,
        },
    );
    assert_eq!(count(&s.run(100), ME.0, "MOVE_TO_LOCATION"), 0);
    s.run(60_100);
    assert!(!s.world.is_control_blocked(ME));
}

#[test]
fn fear_makes_a_monster_run_from_the_effector() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    let before = s.world.position_of(hare).unwrap();
    s.apply(ME, hare, 9106);
    s.run(2_000);
    let after = s.world.position_of(hare).unwrap();
    // The hare stands east of the caster: it runs east.
    assert!(after.0 > before.0, "{before:?} -> {after:?}");
}

#[test]
fn runaway_gives_the_monster_a_fear_time_and_the_flee_effect() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(ME, hare, 9131);
    assert_eq!(s.world.fear_time(hare), 10);
    // The fear task starts at once and shows the flee effect.
    s.run(200);
    assert_ne!(s.visual(hare) & mask(AbnormalVisualEffect::TurnFlee), 0);
    // Each run takes `FEAR_TICKS` (5) seconds off; after ten seconds the fear is over.
    s.run(12_000);
    assert!(s.world.fear_time(hare) <= 0);
    assert_eq!(s.visual(hare) & mask(AbnormalVisualEffect::TurnFlee), 0);
}

#[test]
fn confuse_sets_the_monster_on_somebody_it_sees() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(ME, hare, 9132);
    // The target is a random visible creature: one of the players. (Without anything on its
    // aggro list `thinkAttack` sends the monster back to ACTIVE at once, as in Java.)
    let target = s.world.creature_target(hare).unwrap();
    assert!(target == ME || target == OTHER);
    // The effect is instant: it leaves nothing in the effect list.
    assert!(s.world.effect_list(hare).is_none_or(EffectList::is_empty));
}

#[test]
fn betray_needs_a_summon_and_never_starts_on_a_player() {
    let mut s = Scene::new();
    s.apply(ME, OTHER, 9142);
    assert!(!s.world.is_betrayed(OTHER));
    assert!(!s.world.is_affected(OTHER, effect_flag::BETRAYED));
}

// ---- mutes -----------------------------------------------------------------------------------------

#[test]
fn mutes_set_their_flags_and_the_physical_attack_mute_blocks_attacks() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(hare, ME, 9107);
    assert!(s.world.is_muted(ME) && !s.world.is_physical_muted(ME) && !s.world.is_disabled(ME));
    s.apply(hare, ME, 9108);
    assert!(s.world.is_physical_muted(ME) && !s.world.is_disabled(ME));
    s.apply(hare, ME, 9109);
    // `isDisabled()` includes `isPhysicalAttackMuted()`: no auto attacks.
    assert!(s.world.is_physical_attack_muted(ME) && s.world.is_disabled(ME));
    assert!(s.world.is_attack_disabled(ME));
    s.run(60_100);
    assert!(!s.world.is_muted(ME) && !s.world.is_physical_muted(ME) && !s.world.is_disabled(ME));
}

// ---- disarm ----------------------------------------------------------------------------------------

#[test]
fn disarm_takes_the_weapon_off_and_gives_it_back() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    assert!(s.world.paperdoll_object(ME, paperdoll::RHAND).is_some());
    s.log.take();
    s.apply(hare, ME, 9110);
    assert!(s.world.is_disarmed(ME));
    assert!(s.world.paperdoll_object(ME, paperdoll::RHAND).is_none());
    let sent = s.run(300);
    assert!(count(&sent, ME.0, "INVENTORY_UPDATE") >= 1);
    assert!(count(&sent, ME.0, "USER_INFO") >= 1);
    // A disarmed player cannot put a weapon on (UseItem).
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: 0x3000_0001,
            ctrl_pressed: false,
        },
    );
    let sent = s.run(300);
    assert!(
        message_ids(&sent, ME.0).contains(&SystemMessageId::YouDoNotMeetTheRequiredConditionToEquipThatItem)
    );
    assert!(s.world.paperdoll_object(ME, paperdoll::RHAND).is_none());
    // It ends: the same weapon is equipped again.
    s.run(60_100);
    assert!(!s.world.is_disarmed(ME));
    assert_eq!(
        s.world.paperdoll_object(ME, paperdoll::RHAND),
        Some(ObjectId(0x3000_0001))
    );
}

// ---- invincible, hide, flags -----------------------------------------------------------------------

#[test]
fn invincible_makes_the_creature_immune_to_damage() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(hare, ME, 9111);
    assert!(s.world.is_invul(ME));
    s.world.reduce_current_hp(ME, 100.0, Some(hare));
    assert_eq!(s.world.player(ME).unwrap().core.cur_hp, 5000.0);
    // Negative skills do not land on an invulnerable creature either (`applyEffects`).
    s.apply(hare, ME, 9101);
    assert!(!s.world.is_stunned(ME));
    s.run(60_100);
    assert!(!s.world.is_invul(ME));
}

#[test]
fn hide_makes_the_player_invisible_and_monsters_forget_it() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.world.set_creature_target(hare, Some(ME));
    s.world.set_intention_attack(hare, ME);
    s.run(100);
    s.apply(ME, ME, 9112);
    // Others are told to forget the player and see it no more.
    let info: CharInfo = s.world.char_info(ME, OTHER).unwrap();
    assert!(info.invisible);
    assert_eq!(s.world.creature_target(hare), None);
    assert_ne!(s.world.intention_of(hare), Intention::Attack);
    // The player's own UserInfo carries the stealth effect.
    let user = s.world.user_info(ME).unwrap();
    assert_ne!(
        user.abnormal_visual_effects & mask(AbnormalVisualEffect::Stealth),
        0
    );
    s.run(60_100);
    assert!(!s.world.char_info(ME, OTHER).unwrap().invisible);
}

#[test]
fn flag_flags_the_player_for_pvp_while_it_lasts() {
    let mut s = Scene::new();
    s.apply(OTHER, ME, 9121);
    assert_eq!(s.world.pvp_flag_of(ME), 1);
    assert_eq!(s.world.user_info(ME).unwrap().pvp_flag, 1);
    // Flag only starts on a player.
    let hare = s.hare(100);
    s.apply(ME, hare, 9121);
    assert_eq!(s.world.pvp_flag_of(hare), 0);
    s.run(60_100);
    assert_eq!(s.world.pvp_flag_of(ME), 0);
}

#[test]
fn the_blessings_and_resurrection_block_are_flags() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(ME, ME, 9137);
    s.apply(ME, ME, 9138);
    s.apply(ME, ME, 9139);
    assert!(s.world.is_affected(ME, effect_flag::PROTECTION_BLESSING));
    assert!(s.world.is_affected(ME, effect_flag::NOBLESS_BLESSING));
    assert!(s.world.is_affected(ME, effect_flag::BLOCK_RESURRECTION));
    // ProtectionBlessing and Lucky start on players only; NoblesseBless on playables.
    s.apply(ME, hare, 9137);
    s.apply(ME, hare, 9138);
    s.apply(ME, hare, 9147);
    assert!(!s.world.is_affected(hare, effect_flag::PROTECTION_BLESSING));
    assert!(!s.world.is_affected(hare, effect_flag::NOBLESS_BLESSING));
    assert!(
        s.world
            .effect_list(hare)
            .unwrap()
            .buffs()
            .iter()
            .all(|i| i.effects().is_empty())
    );
    // Noblesse blessing keeps the buffs on death and ends itself (`Playable.doDie`).
    s.apply(ME, ME, 9149);
    s.world.do_die(ME, Some(hare));
    assert!(!s.world.is_affected(ME, effect_flag::NOBLESS_BLESSING));
    assert!(s.world.is_affected_by_skill(ME, 9149));
    s.apply(ME, OTHER, 9141);
    assert!(
        s.world
            .effect_list(OTHER)
            .unwrap()
            .first_effect(EffectType::Debuff)
            .is_some()
    );
}

// ---- the toggles: silent move, fake death, relax ---------------------------------------------------

#[test]
fn silent_move_drains_mp_each_tick_and_hides_from_monsters() {
    let mut s = Scene::new();
    s.apply(ME, ME, 9113);
    assert!(s.world.is_affected(ME, effect_flag::SILENT_MOVE));
    let before = s.world.player(ME).unwrap().core.cur_mp;
    s.run(1_400);
    let after = s.world.player(ME).unwrap().core.cur_mp;
    // 10 per second of ticks * 666 ms = 6.66 per tick, two ticks.
    assert!(after < before - 10.0, "{before} -> {after}");
    assert!(s.world.is_affected_by_skill(ME, 9113));
    // Without the MP the toggle goes with "your skill was deactivated due to lack of MP".
    s.world.reduce_mp(ME, 1000.0);
    let sent = s.run(1_000);
    assert!(message_ids(&sent, ME.0).contains(&SystemMessageId::YourSkillWasDeactivatedDueToLackOfMp));
    assert!(!s.world.is_affected(ME, effect_flag::SILENT_MOVE));
}

#[test]
fn a_player_in_silent_move_is_not_aggroed_by_monsters() {
    let mut s = Scene::new();
    let wolf = s.world.spawn_npc_at(9001, X + 60, Y, Z, 0).unwrap();
    s.run(30_000);
    // Without silent move the aggressive wolf attacks the nearby player.
    assert_eq!(s.world.intention_of(wolf), Intention::Attack);

    let mut quiet = Scene::new();
    quiet.apply(ME, ME, 9150);
    quiet.apply(ME, OTHER, 9150);
    let wolf = quiet.world.spawn_npc_at(9001, X + 60, Y, Z, 0).unwrap();
    quiet.run(30_000);
    assert_ne!(quiet.world.intention_of(wolf), Intention::Attack);
}

#[test]
fn fake_death_plays_dead_and_getting_up_gives_a_grace_period() {
    let mut s = Scene::with(EffectSettings {
        fake_death_up_protection: 5,
        ..EffectSettings::default()
    });
    let hare = s.hare(100);
    s.log.take();
    s.apply(ME, ME, 9114);
    assert!(s.world.is_fake_death(ME) && s.world.is_alike_dead(ME) && !s.world.is_dead(ME));
    let sent = s.run(100);
    let wait: ChangeWaitType = decode(of(&sent, ME.0, "CHANGE_WAIT_TYPE")[0]);
    assert_eq!(wait.move_type, ChangeWaitType::WT_START_FAKEDEATH);
    // A fake dead player is no target for the monsters (and nobody can hit it as a living one).
    assert!(s.world.is_attack_disabled(ME));

    // Getting up: both packets, and the monsters leave the player alone for a while.
    s.log.take();
    s.world.stop_skill_effects(ME, SkillFinishType::Removed, 9114);
    let sent = s.run(100);
    let wait: ChangeWaitType = decode(of(&sent, ME.0, "CHANGE_WAIT_TYPE")[0]);
    assert_eq!(wait.move_type, ChangeWaitType::WT_STOP_FAKEDEATH);
    assert_eq!(count(&sent, ME.0, "REVIVE"), 1);
    assert!(!s.world.is_fake_death(ME));
    assert!(s.world.is_recent_fake_death(ME));
    s.run(5_500);
    assert!(!s.world.is_recent_fake_death(ME));
    let _ = hare;
}

#[test]
fn damage_makes_a_fake_dead_player_stand_up() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(ME, ME, 9114);
    assert!(s.world.is_fake_death(ME));
    // `notifyDamageReceived`: FakeDeathDamageStand is on by default.
    s.world.notify_damage_received(ME, 10.0);
    assert!(!s.world.is_fake_death(ME));
    assert!(!s.world.is_affected_by_skill(ME, 9114));
    let _ = hare;
}

#[test]
fn relax_sits_the_player_down_and_a_full_hp_player_gets_up_again() {
    let mut s = Scene::new();
    s.log.take();
    s.apply(ME, ME, 9115);
    assert!(s.world.is_sitting(ME));
    let sent = s.run(100);
    let wait: ChangeWaitType = decode(of(&sent, ME.0, "CHANGE_WAIT_TYPE")[0]);
    assert_eq!(wait.move_type, ChangeWaitType::WT_SITTING);
    // After the sit animation the sitting player is held in place (`SitDownTask`).
    let mut sent = s.run(2_500);
    assert!(s.world.is_movement_disabled(ME));
    // The toggle runs out because the HP is full ("deactivated as HP was fully recovered").
    sent.extend(s.run(1_000));
    assert!(
        message_ids(&sent, ME.0).contains(&SystemMessageId::ThatSkillHasBeenDeActivatedAsHpWasFullyRecovered)
    );
    assert!(!s.world.is_affected_by_skill(ME, 9115));
    // Standing up takes the same 2.5 s.
    s.world.stand_up(ME);
    s.run(2_600);
    assert!(!s.world.is_sitting(ME) && !s.world.is_movement_disabled(ME));
}

#[test]
fn chameleon_rest_is_relax_with_silent_move() {
    let mut s = Scene::new();
    s.apply(ME, ME, 9140);
    assert!(s.world.is_sitting(ME));
    assert!(s.world.is_affected(ME, effect_flag::RELAXING));
    assert!(s.world.is_affected(ME, effect_flag::SILENT_MOVE));
    // NOTE(java-quirk): no HP check, so it stays while sitting.
    s.run(3_000);
    assert!(s.world.is_affected_by_skill(ME, 9140));
    // Standing up stops the relaxing effects.
    s.world.stand_up(ME);
    assert!(!s.world.is_affected_by_skill(ME, 9140));
}

// ---- blocks ----------------------------------------------------------------------------------------

#[test]
fn block_chat_bans_the_chat_for_as_long_as_it_lasts() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(hare, ME, 9133);
    assert!(s.world.is_chat_banned(ME));
    s.log.take();
    say(&mut s, "hello");
    let sent = s.run(100);
    assert_eq!(count(&sent, OTHER.0, "SAY2"), 0);
    assert!(
        message_ids(&sent, ME.0)
            .contains(&SystemMessageId::YouHaveBeenReportedAsAnIllegalProgramUserSoChattingIsNotAllowed)
    );
    s.run(60_100);
    assert!(!s.world.is_chat_banned(ME));
    say(&mut s, "hello");
    assert_eq!(count(&s.run(100), OTHER.0, "SAY2"), 1);
}

#[test]
fn block_action_refuses_attack_clicks_and_bans_chat_and_party() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(hare, ME, 9134);
    assert!(s.world.is_chat_banned(ME) && s.world.is_party_banned(ME));
    s.log.take();
    // `AttackRequest`: `checkCondition(-1)` fails.
    s.input(
        ME,
        ClientInput::AttackRequest {
            object_id: hare.0,
            attack_id: 0,
        },
    );
    let sent = s.run(100);
    assert!(message_ids(&sent, ME.0).contains(
        &SystemMessageId::YouHaveBeenReportedAsAnIllegalProgramUserSoYourActionsHaveBeenRestricted
    ));
    assert_eq!(count(&sent, ME.0, "ATTACK"), 0);
    // `Action` (-4) is not blocked by this skill: it targets the hare.
    s.input(
        ME,
        ClientInput::Action {
            object_id: hare.0,
            action_id: 0,
        },
    );
    s.run(100);
    assert_eq!(s.world.target_of(ME), Some(hare));
    s.run(60_100);
    assert!(!s.world.is_chat_banned(ME) && !s.world.is_party_banned(ME));
}

#[test]
fn block_abnormal_slot_refuses_skills_of_those_types_and_gives_them_back() {
    let mut s = Scene::new();
    s.apply(ME, ME, 9116);
    let list = s.world.effect_list(ME).unwrap();
    assert!(list.blocked_abnormal_types().iter().any(|t| t.name() == "PD_UP"));
    assert!(list.blocked_abnormal_types().iter().any(|t| t.name() == "MD_UP"));
    // A PD_UP buff does not land.
    s.apply(ME, ME, 9149);
    assert!(!s.world.is_affected_by_skill(ME, 9149));
    s.run(60_100);
    assert!(
        s.world
            .effect_list(ME)
            .unwrap()
            .blocked_abnormal_types()
            .is_empty()
    );
    s.apply(ME, ME, 9149);
    assert!(s.world.is_affected_by_skill(ME, 9149));
}

#[test]
fn an_unknown_abnormal_slot_name_blocks_none() {
    // `AbnormalType.getAbnormalType` returns NONE for a name it does not know.
    let mut s = Scene::new();
    s.apply(ME, ME, 9148);
    let list = s.world.effect_list(ME).unwrap();
    assert_eq!(list.blocked_abnormal_types().len(), 1);
    assert_eq!(
        list.blocked_abnormal_types().iter().next().unwrap().name(),
        "NONE"
    );
}

#[test]
fn enlarge_abnormal_slot_adds_buff_slots_until_it_ends() {
    let mut s = Scene::new();
    let before = s.world.max_buff_count(ME);
    s.apply(ME, ME, 9117);
    assert_eq!(s.world.max_buff_count(ME), before + 4);
    // On a monster it does not start (`effected.isPlayer()`).
    let hare = s.hare(100);
    let hare_before = s.world.max_buff_count(hare);
    s.apply(ME, hare, 9117);
    assert_eq!(s.world.max_buff_count(hare), hare_before);
    s.run(60_100);
    assert_eq!(s.world.max_buff_count(ME), before);
}

#[test]
fn resist_skill_makes_the_creature_immune_to_that_skill() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(hare, ME, 9118);
    assert!(s.world.is_invul_against(ME, 9101, 1));
    assert!(!s.world.is_invul_against(ME, 9101, 2));
    s.apply(hare, ME, 9101);
    assert!(!s.world.is_stunned(ME));
    s.run(60_100);
    assert!(!s.world.is_invul_against(ME, 9101, 1));
    s.apply(hare, ME, 9101);
    assert!(s.world.is_stunned(ME));
}

#[test]
fn immobile_buff_immobilizes_while_it_lasts() {
    let mut s = Scene::new();
    s.apply(ME, ME, 9135);
    assert!(s.world.is_immobilized(ME) && s.world.is_movement_disabled(ME));
    assert!(!s.world.is_disabled(ME));
    s.run(60_100);
    assert!(!s.world.is_immobilized(ME));
}

// ---- traits ----------------------------------------------------------------------------------------

#[test]
fn attack_and_defence_traits_move_the_tables_and_unwind() {
    let mut s = Scene::new();
    s.apply(ME, ME, 9119);
    // `(50 + 100) / 100` as a float.
    assert_eq!(s.world.attack_trait(ME, TraitType::Shock), 1.5);
    s.apply(ME, ME, 9120);
    // 100% is immunity, 20% a scaled defence.
    assert!(s.world.is_trait_invul(ME, TraitType::Sleep));
    assert_eq!(s.world.defence_trait(ME, TraitType::Hold), 1.2);
    s.run(60_100);
    assert_eq!(s.world.attack_trait(ME, TraitType::Shock), 1.0);
    assert!(!s.world.is_trait_invul(ME, TraitType::Sleep));
    assert_eq!(s.world.defence_trait(ME, TraitType::Hold), 1.0);
}

// ---- instant effects on the target -----------------------------------------------------------------

#[test]
fn target_cancel_makes_the_target_forget_what_it_was_doing() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.world.set_creature_target(hare, Some(ME));
    s.world.set_intention_attack(hare, ME);
    s.run(100);
    s.apply(ME, hare, 9122);
    assert_eq!(s.world.creature_target(hare), None);
    assert_ne!(s.world.intention_of(hare), Intention::Attack);
    // It also works on a player: the target is dropped.
    s.world.set_target(OTHER, Some(ME));
    s.apply(ME, OTHER, 9122);
    assert_eq!(s.world.target_of(OTHER), None);
}

#[test]
fn bluff_turns_the_target_around_with_the_two_rotation_packets() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    // The hare faces another way than the caster (heading 0).
    let facing = s.world.spawn_npc_at(9002, X + 100, Y + 80, Z, 16_384).unwrap();
    s.run(100);
    s.log.take();
    s.apply(ME, facing, 9123);
    let sent = s.run(100);
    let start: StartRotation = decode(of(&sent, ME.0, "START_ROTATING")[0]);
    let stop: StopRotation = decode(of(&sent, ME.0, "FINISH_ROTATING")[0]);
    assert_eq!(
        (start.object_id, start.degree, start.side, start.speed),
        (facing.0, 16_384, 1, 65535)
    );
    assert_eq!((stop.object_id, stop.degree, stop.speed), (facing.0, 0, 65535));
    assert_eq!(s.world.npc(facing).unwrap().core.heading, 0);
    let _ = hare;
}

#[test]
fn skill_turning_is_an_instant_effect_that_breaks_no_cast_when_none_runs() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(ME, hare, 9124);
    assert!(s.world.effect_list(hare).is_none_or(EffectList::is_empty));
}

#[test]
fn enemy_charge_rushes_the_caster_to_the_target() {
    let mut s = Scene::new();
    let hare = s.world.spawn_npc_at(9002, X + 800, Y, Z, 0).unwrap();
    s.run(100);
    s.log.take();
    s.apply(ME, hare, 9125);
    let sent = s.run(100);
    let fly: FlyToLocation = decode(of(&sent, ME.0, "FLY_TO_LOCATION")[0]);
    assert_eq!((fly.org_x, fly.org_y), (X, Y));
    assert_eq!(fly.fly_type, 3);
    // `flyRadius` is 0: the offset is 30, the caster stops 30 short of the target.
    assert_eq!((fly.dest_x, fly.dest_y), (X + 770, Y));
    assert_eq!(s.world.position_of(ME).unwrap().0, X + 770);
    assert_eq!(count(&sent, ME.0, "VALIDATE_LOCATION"), 1);
}

#[test]
fn enemy_charge_does_nothing_when_the_caster_cannot_move() {
    let mut s = Scene::new();
    let hare = s.world.spawn_npc_at(9002, X + 800, Y, Z, 0).unwrap();
    s.apply(hare, ME, 9103);
    s.log.take();
    s.apply(ME, hare, 9125);
    assert_eq!(count(&s.run(100), ME.0, "FLY_TO_LOCATION"), 0);
    // And not a distance over 2000.
    s.run(60_100);
    let far = s.world.spawn_npc_at(9002, X + 2500, Y, Z, 0).unwrap();
    s.run(100);
    s.apply(ME, far, 9125);
    assert_eq!(count(&s.run(100), ME.0, "FLY_TO_LOCATION"), 0);
}

// ---- charges and souls -----------------------------------------------------------------------------

#[test]
fn focus_energy_raises_the_charges_up_to_its_maximum() {
    let mut s = Scene::new();
    s.log.take();
    s.apply(OTHER, ME, 9126);
    assert_eq!(s.world.charges_of(ME), 1);
    let sent = s.run(100);
    let etc: EtcStatusUpdate = decode(of(&sent, ME.0, "ETC_STATUS_UPDATE").last().unwrap());
    assert_eq!(etc.charges, 1);
    assert_eq!(
        message_ids(&sent, ME.0),
        [SystemMessageId::YourForceHasIncreasedToLevelS1]
    );
    s.apply(OTHER, ME, 9126);
    assert_eq!(s.world.charges_of(ME), 2);
    // `charge` is the maximum (2): the third one only tells it is full.
    s.log.take();
    s.apply(OTHER, ME, 9126);
    assert_eq!(s.world.charges_of(ME), 2);
    assert_eq!(
        message_ids(&s.run(100), ME.0),
        [SystemMessageId::YourForceHasReachedMaximumCapacity]
    );
    // Ten minutes without a new charge clear them (`ResetChargesTask`).
    s.run(600_100);
    assert_eq!(s.world.charges_of(ME), 0);
}

#[test]
fn charges_can_be_spent_and_the_reset_task_follows() {
    let mut s = Scene::new();
    s.world.increase_charges(ME, 3, 5);
    assert_eq!(s.world.charges_of(ME), 3);
    assert!(!s.world.decrease_charges(ME, 4));
    assert!(s.world.decrease_charges(ME, 3));
    assert_eq!(s.world.charges_of(ME), 0);
    s.world.increase_souls(ME, 2);
    assert_eq!(s.world.charged_souls_of(ME), 2);
    assert!(s.world.decrease_souls(ME, 5));
    assert_eq!(s.world.charged_souls_of(ME), 0);
    s.world.increase_souls(ME, 4);
    s.run(600_100);
    assert_eq!(s.world.charged_souls_of(ME), 0);
}

#[test]
fn the_conditions_read_the_effects_charges_and_souls() {
    let mut s = Scene::new();
    s.world.increase_charges(ME, 2, 5);
    s.world.increase_souls(ME, 3);
    let view = s.world.stats(ME).unwrap();
    let _ = view;
    assert_eq!(s.world.charges_of(ME), 2);
    assert_eq!(s.world.charged_souls_of(ME), 3);
}

#[test]
fn soul_eating_listens_for_big_experience_gains_while_it_lasts() {
    let mut s = Scene::new();
    s.apply(OTHER, ME, 9129);
    s.log.take();
    // The player has no Max Souls stat, so the soul cannot be absorbed (the message, no soul).
    s.world.add_exp(ME, 10);
    let sent = s.run(100);
    assert!(message_ids(&sent, ME.0).contains(&SystemMessageId::SoulCannotBeAbsorbedAnymore));
    // A gain under `expNeeded` is ignored.
    s.log.take();
    s.world.add_exp(ME, 1);
    assert!(!message_ids(&s.run(100), ME.0).contains(&SystemMessageId::SoulCannotBeAbsorbedAnymore));
    // Once the effect has ended nobody listens.
    s.run(60_100);
    s.log.take();
    s.world.add_exp(ME, 10);
    assert!(!message_ids(&s.run(100), ME.0).contains(&SystemMessageId::SoulCannotBeAbsorbedAnymore));
}

// ---- passives --------------------------------------------------------------------------------------

#[test]
fn the_slot_passives_set_and_clear_the_player_values() {
    let mut s = Scene::new();
    assert!(s.world.add_skill(ME, 9143, 1));
    assert!(s.world.add_skill(ME, 9144, 1));
    assert!(s.world.add_skill(ME, 9145, 1));
    let view = s.world.stats(ME).unwrap();
    assert_eq!(view.talisman_slots(), 3);
    assert!(view.can_equip_cloak());
    assert_eq!(view.expertise_penalty_bonus(), 2);
    // They start on players only.
    let hare = s.hare(100);
    s.world.add_skill(hare, 9143, 1);
    assert_eq!(s.world.stats(hare).unwrap().talisman_slots(), 0);
    // Replacing a passive undoes its effect first (`removeSkill` then `addSkill`).
    s.world.remove_skill(ME, 9143, true);
    assert_eq!(s.world.stats(ME).unwrap().talisman_slots(), 0);
    s.world.remove_skill(ME, 9145, true);
    assert_eq!(s.world.stats(ME).unwrap().expertise_penalty_bonus(), 0);
}

#[test]
fn passive_stops_a_monster_from_being_aggressive() {
    let mut s = Scene::new();
    let wolf = s.world.spawn_npc_at(9001, X + 5_000, Y, Z, 0).unwrap();
    s.run(100);
    s.apply(ME, wolf, 9136);
    assert!(s.world.is_affected(wolf, effect_flag::PASSIVE));
    // It starts on attackable NPCs only.
    s.apply(ME, OTHER, 9136);
    assert!(!s.world.is_affected(OTHER, effect_flag::PASSIVE));
    // A passive wolf does not attack the player who comes next to it, an ordinary one does.
    let ordinary = s.world.spawn_npc_at(9001, X + 5_000, Y + 2_000, Z, 0).unwrap();
    s.run(100);
    s.enter(ObjectId(0x2000_0003), "Near", (X + 5_050, Y, Z), vec![]);
    s.enter(ObjectId(0x2000_0004), "Near2", (X + 5_050, Y + 2_000, Z), vec![]);
    s.run(30_000);
    assert_ne!(s.world.intention_of(wolf), Intention::Attack);
    assert_eq!(s.world.intention_of(ordinary), Intention::Attack);
    s.run(60_000);
    assert!(!s.world.is_affected(wolf, effect_flag::PASSIVE));
}

#[test]
fn distrust_turns_a_monster_on_another_one_and_back() {
    let mut s = Scene::new();
    let a = s.hare(100);
    let b = s.hare(300);
    s.apply(ME, a, 9130);
    assert_eq!(s.world.distrust_target(a), Some(b));
    // Hit by the player, the monster fights the other monster all the same.
    s.world.set_intention_attack(a, ME);
    s.run(1_000);
    assert_eq!(s.world.attack_target_of(a), Some(b));
    s.run(60_100);
    assert_eq!(s.world.distrust_target(a), None);
    // A player is no valid target (`effected.isMonster()`): the caster is told so.
    s.log.take();
    s.apply(ME, OTHER, 9130);
    assert_eq!(message_ids(&s.run(100), ME.0), [SystemMessageId::InvalidTarget]);
}
