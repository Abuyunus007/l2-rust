#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The effect handlers that needed casting or new world parts (M5-EFX-05), through the world with
//! recording clients: `CallSkill` and the four `TriggerSkillBy*` effects with the creature events
//! that feed them, `TargetMe` and the locked target, `Resurrection` / `ResurrectionSpecial` with
//! the `ConfirmDlg` / `DlgAnswer` round trip, `NevitsHourglass`, `OpenChest`, `ConvertItem`,
//! `Grow`, `RestorationRandom`, the `FlyToLocation` of the jump effects and the `CancelReturn` of
//! the dispels. The skills are synthetic (named after the Java class); the expected behaviour comes
//! from `dist/game/data/scripts/handlers/skill/effects/*.java` and the `Creature` / `Player`
//! methods named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{
    ConfirmDlg, FlyToLocation, MagicSkillLaunched, MagicSkillUse, SocialAction, SystemMessage,
    SystemMessageParam,
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
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
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

/// Item 2 turns into item 3 (`change_weaponId`) with `ConvertItem`; 1344 is a stackable arrow.
const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="change_weaponId" val="3" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
<item id="3" type="Weapon" name="Changed Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<stats><stat type="pAtk">55</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
<item id="1344" type="EtcItem" name="Mithril Arrow"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="1345" type="EtcItem" name="Shining Arrow"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
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
    <collision><radius normal="10" grown="25"/><height normal="20"/></collision>
  </npc>
  <npc id="9003" type="Chest" level="5" name="Chest">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="0" isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9004" type="Chest" level="40" name="Strong Chest">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="0" isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
</list>"#;

/// The triggered skill: a plain buff with a long reuse (9303 has a 5 s reuse delay).
const TRIGGERED: i32 = 9302;
const TRIGGERED_WITH_REUSE: i32 = 9303;

/// One synthetic skill: `kind` is the operate type, `abnormal` its abnormal type, `effects` the
/// inner xml of `<effects>`, `extra` more skill elements.
fn skill_with(id: i32, kind: &str, abnormal: &str, effect_point: i32, extra: &str, effects: &str) -> String {
    let abnormal = if abnormal.is_empty() {
        String::new()
    } else {
        format!(
            "<abnormalTime>60</abnormalTime><abnormalType>{abnormal}</abnormalType><abnormalLevel>1</abnormalLevel>"
        )
    };
    format!(
        r#"<skill id="{id}" levels="1" name="S{id}"><operateType>{kind}</operateType><targetType>ONE</targetType><effectPoint>{effect_point}</effectPoint>{abnormal}{extra}<effects>{effects}</effects></skill>"#
    )
}

/// A trigger buff: `kind` of the effect, its parameters, an abnormal type of its own.
fn trigger(id: i32, abnormal: &str, effect: &str, params: &str) -> String {
    skill_with(
        id,
        "A2",
        abnormal,
        0,
        "",
        &format!(r#"<effect name="{effect}">{params}</effect>"#),
    )
}

fn skills_xml() -> String {
    let list = [
        // The skill the triggers cast: a buff of abnormal type PD_UP.
        skill_with(TRIGGERED, "A2", "PD_UP", 0, "", r#"<effect name="Buff"/>"#),
        skill_with(
            TRIGGERED_WITH_REUSE,
            "A2",
            "MD_UP",
            0,
            "<reuseDelay>5000</reuseDelay>",
            r#"<effect name="Buff"/>"#,
        ),
        // CallSkill: 9301 calls 9302, 9304 calls itself, 9305 calls the skill with the reuse.
        skill_with(
            9301,
            "A1",
            "",
            0,
            "",
            r#"<effect name="CallSkill"><skillId>9302</skillId><skillLevel>1</skillLevel></effect>"#,
        ),
        skill_with(
            9304,
            "A1",
            "",
            0,
            "",
            r#"<effect name="CallSkill"><skillId>9304</skillId><skillLevel>1</skillLevel></effect>"#,
        ),
        skill_with(
            9305,
            "A1",
            "",
            0,
            "",
            r#"<effect name="CallSkill"><skillId>9303</skillId></effect>"#,
        ),
        // `params.getInt("skillId")` has no default: the constructor throws and the effect is dropped.
        skill_with(9399, "A1", "", 0, "", r#"<effect name="CallSkill"/>"#),
        // The trigger buffs.
        trigger(
            9310,
            "MULTI_BUFF",
            "TriggerSkillByDamageDealt",
            "<skillId>9302</skillId><skillLevel>1</skillLevel><targetType>SELF</targetType>",
        ),
        trigger(
            9311,
            "MULTI_BUFF_A",
            "TriggerSkillByDamageDealt",
            "<skillId>9302</skillId><targetType>SELF</targetType><isCritical>true</isCritical>",
        ),
        trigger(
            9312,
            "ATTACK_TIME_UP",
            "TriggerSkillByDamageDealt",
            "<skillId>9302</skillId><targetType>SELF</targetType><minDamage>50</minDamage>",
        ),
        trigger(
            9313,
            "CRITICAL_PROB_UP",
            "TriggerSkillByDamageDealt",
            "<skillId>9302</skillId><targetType>SELF</targetType><attackerType>Attackable</attackerType>",
        ),
        trigger(
            9314,
            "HP_REGEN_UP",
            "TriggerSkillByDamageDealt",
            "<skillId>9302</skillId><targetType>SELF</targetType><allowWeapons>BOW</allowWeapons>",
        ),
        trigger(
            9315,
            "SPEED_UP",
            "TriggerSkillByDamageDealt",
            "<skillId>9302</skillId><targetType>SELF</targetType><allowWeapons>SWORD,BLUNT</allowWeapons>",
        ),
        trigger(
            9316,
            "PD_UP_BOW",
            "TriggerSkillByDamageDealt",
            "<skillId>9302</skillId><targetType>SELF</targetType><chance>0</chance>",
        ),
        trigger(
            9320,
            "MULTI_BUFF",
            "TriggerSkillByDamageReceived",
            "<skillId>9302</skillId><targetType>SELF</targetType>",
        ),
        trigger(
            9321,
            "MULTI_BUFF_A",
            "TriggerSkillByDamageReceived",
            "<skillId>9302</skillId><targetType>SELF</targetType><minAttackerLevel>50</minAttackerLevel>",
        ),
        trigger(
            9330,
            "MULTI_BUFF",
            "TriggerSkillByAvoid",
            "<skillId>9302</skillId><skillLevel>1</skillLevel><targetType>SELF</targetType>",
        ),
        trigger(
            9340,
            "MULTI_BUFF",
            "TriggerSkillBySkill",
            "<castSkillId>9341</castSkillId><skillId>9302</skillId><skillLevel>1</skillLevel><targetType>SELF</targetType>",
        ),
        // The skill ME casts to feed the BySkill trigger.
        skill_with(9341, "A1", "", 0, "", ""),
        // TargetMe (not harmful, so `checkPvpSkill` lets the target change).
        skill_with(9350, "A2", "TARGET_LOCK", 0, "", r#"<effect name="TargetMe"/>"#),
        skill_with(
            9351,
            "A1",
            "",
            0,
            "",
            r#"<effect name="TargetMeProbability"><chance>100</chance></effect>"#,
        ),
        // Resurrection.
        skill_with(
            9360,
            "A1",
            "",
            0,
            "",
            r#"<effect name="Resurrection"><power>50</power></effect>"#,
        ),
        skill_with(
            9361,
            "A2",
            "PROTECTION",
            0,
            "",
            r#"<effect name="ResurrectionSpecial"><power>0</power></effect>"#,
        ),
        skill_with(
            9362,
            "A2",
            "BLOCK_RESURRECTION",
            0,
            "<stayAfterDeath>true</stayAfterDeath>",
            r#"<effect name="BlockResurrection"/>"#,
        ),
        // NevitsHourglass, OpenChest, ConvertItem, Grow.
        skill_with(
            9370,
            "A2",
            "REFLECT_ABNORMAL",
            0,
            "",
            r#"<effect name="NevitsHourglass"/>"#,
        ),
        skill_with(9371, "A1", "", 0, "", r#"<effect name="OpenChest"/>"#),
        skill_with(9372, "A1", "", 0, "", r#"<effect name="ConvertItem"/>"#),
        skill_with(9373, "A2", "BIG_BODY", 0, "", r#"<effect name="Grow"/>"#),
        // RestorationRandom: always the first group.
        skill_with(
            9374,
            "A1",
            "",
            0,
            "",
            r#"<effect name="RestorationRandom"><items><item chance="100"><item id="1344" count="10"/><item id="1345" count="3"/></item></items></effect>"#,
        ),
        skill_with(
            9375,
            "A1",
            "",
            0,
            "",
            r#"<effect name="RestorationRandom"><items><item chance="0"><item id="1344" count="10"/></item></items></effect>"#,
        ),
        // The jump effects.
        skill_with(
            9380,
            "A1",
            "",
            0,
            "<flyRadius>200</flyRadius><flyCourse>0</flyCourse>",
            r#"<effect name="Blink"/>"#,
        ),
        skill_with(
            9381,
            "A1",
            "",
            -100,
            "<flyRadius>100</flyRadius>",
            r#"<effect name="ThrowUp"/>"#,
        ),
        skill_with(9382, "A1", "", 0, "", r#"<effect name="TeleportToTarget"/>"#),
        // The dispels, with a buff to take.
        skill_with(9390, "A1", "", -100, "", r#"<effect name="DispelAll"/>"#),
        skill_with(
            9391,
            "A1",
            "",
            -100,
            "",
            r#"<effect name="DispelBySlotProbability"><dispel>PD_UP,1</dispel><rate>100</rate></effect>"#,
        ),
    ];
    format!("<list>{}</list>", list.concat())
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const OTHER: ObjectId = ObjectId(0x2000_0002);
const OLD_SWORD: ObjectId = ObjectId(0x3000_0001);

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

/// A +3 sword in the right hand.
fn sword() -> Vec<ItemData> {
    vec![ItemData {
        object_id: OLD_SWORD,
        item_id: 2,
        count: 1,
        enchant_level: 3,
        loc: "PAPERDOLL".into(),
        loc_data: i32::try_from(paperdoll::RHAND).unwrap(),
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
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

    fn npc(&mut self, npc_id: i32, dx: i32) -> ObjectId {
        let id = self.world.spawn_npc_at(npc_id, X + dx, Y + 50, Z, 0).unwrap();
        self.run(100);
        id
    }

    fn hare(&mut self, dx: i32) -> ObjectId {
        self.npc(9002, dx)
    }

    fn apply(&mut self, from: ObjectId, to: ObjectId, skill: i32) {
        self.world.apply_effects(from, to, SkillKey::new(skill, 1));
    }

    /// ME wears the trigger buff `skill`, put on by itself.
    fn arm(&mut self, skill: i32) {
        self.apply(ME, ME, skill);
        assert!(self.world.is_affected_by_skill(ME, skill), "{skill} did not land");
    }

    fn has(&self, id: ObjectId, skill: i32) -> bool {
        self.world.is_affected_by_skill(id, skill)
    }

    fn input(&mut self, id: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id.0).unwrap()),
            input,
        });
    }

    /// Removes the triggered buff so the next event can be checked on its own.
    fn clear_triggered(&mut self) {
        self.world
            .stop_skill_effects(ME, SkillFinishType::Silent, TRIGGERED);
    }
}

// ---- the registry ----------------------------------------------------------------------------------

#[test]
fn every_handler_of_the_card_is_ported() {
    let registry = l2_world::effects::EffectRegistry::new();
    for name in [
        "CallSkill",
        "TriggerSkillByAvoid",
        "TriggerSkillByDamageDealt",
        "TriggerSkillByDamageReceived",
        "TriggerSkillBySkill",
        "TargetMe",
        "TargetMeProbability",
        "Resurrection",
        "ResurrectionSpecial",
        "NevitsHourglass",
        "OpenChest",
        "ConvertItem",
        "Grow",
        "RestorationRandom",
    ] {
        assert!(registry.is_ported(name), "{name} is not ported");
    }
}

// ---- CallSkill -------------------------------------------------------------------------------------

/// `CallSkill.onStart`: `effector.makeTriggerCast(skill, effected, true)`: the target gets the
/// called skill's effects (here a buff) and everybody sees `MagicSkillUse` / `MagicSkillLaunched`
/// with no cast time.
#[test]
fn call_skill_triggers_the_other_skill_on_the_target() {
    let mut s = Scene::new();
    s.apply(ME, OTHER, 9301);
    assert!(s.has(OTHER, TRIGGERED));
    let sent = s.run(100);
    // Both players see the cast; the animation takes no time (`hitTime = 0`).
    for to in [ME.0, OTHER.0] {
        let uses = of(&sent, to, "MAGIC_SKILL_USE");
        assert_eq!(uses.len(), 1);
        let packet: MagicSkillUse = decode(uses[0]);
        assert_eq!((packet.creature_id, packet.target_id), (ME.0, OTHER.0));
        assert_eq!(
            (packet.skill_id, packet.skill_level, packet.hit_time),
            (9302, 1, 0)
        );
        let launched: MagicSkillLaunched = decode(of(&sent, to, "MAGIC_SKILL_LAUNCHED")[0]);
        assert_eq!(launched.target_ids, vec![OTHER.0]);
    }
}

/// A `CallSkill` without `skillId` is dropped when it is built (Java's constructor throws).
#[test]
fn call_skill_without_a_skill_id_does_nothing() {
    let mut s = Scene::new();
    s.apply(ME, OTHER, 9399);
    assert_eq!(count(&s.run(100), ME.0, "MAGIC_SKILL_USE"), 0);
}

/// "Prevent infinite loop": a `CallSkill` that names its own skill and level does nothing.
#[test]
fn call_skill_does_not_call_itself() {
    let mut s = Scene::new();
    s.apply(ME, OTHER, 9304);
    let sent = s.run(100);
    assert_eq!(count(&sent, ME.0, "MAGIC_SKILL_USE"), 0);
}

/// `makeTriggerCast` disables the skill for its reuse delay (`disableSkill`) and
/// `isSkillDisabled` stops the second call.
#[test]
fn a_triggered_skill_keeps_its_reuse_delay() {
    let mut s = Scene::new();
    s.apply(ME, OTHER, 9305);
    assert!(s.has(OTHER, TRIGGERED_WITH_REUSE));
    s.world
        .stop_skill_effects(OTHER, SkillFinishType::Silent, TRIGGERED_WITH_REUSE);
    s.log.take();
    s.apply(ME, OTHER, 9305);
    assert!(
        !s.has(OTHER, TRIGGERED_WITH_REUSE),
        "the reuse delay did not hold"
    );
    assert_eq!(count(&s.run(100), ME.0, "MAGIC_SKILL_USE"), 0);
}

// ---- TriggerSkillByDamageDealt ---------------------------------------------------------------------

/// The owner of the effect hits somebody: `notifyDamageReceived` fires `ON_CREATURE_DAMAGE_DEALT`
/// for the attacker and the listener casts the skill on the attacker (`SELF`).
#[test]
fn damage_dealt_triggers_the_skill_on_the_attacker() {
    let mut s = Scene::new();
    s.arm(9310);
    let hare = s.hare(0);
    s.world
        .notify_damage_received_by(hare, 10.0, Some(ME), None, false, false);
    assert!(s.has(ME, TRIGGERED));
    // The effect ends: the listener goes with it (`removeListenerIf(owner == this)`).
    s.clear_triggered();
    assert_eq!(s.world.trigger_listener_count(ME), 1);
    s.world.stop_skill_effects(ME, SkillFinishType::Removed, 9310);
    assert_eq!(s.world.trigger_listener_count(ME), 0);
    s.world
        .notify_damage_received_by(hare, 10.0, Some(ME), None, false, false);
    assert!(!s.has(ME, TRIGGERED));
}

/// The conditions of `onAttackEvent`, in Java's words: critical only, minimum damage, attacker
/// type, weapon types, chance 0, and the damage over time and skill hits that are not allowed.
#[test]
fn damage_dealt_checks_its_conditions() {
    let mut s = Scene::new();
    let hare = s.hare(0);
    let hit = |s: &mut Scene, damage: f64, skill: Option<SkillKey>, critical: bool, dot: bool| {
        s.clear_triggered();
        s.world
            .notify_damage_received_by(hare, damage, Some(ME), skill, critical, dot);
        s.has(ME, TRIGGERED)
    };

    // A plain hit passes; a damage over time, a skill hit and a hit on oneself do not.
    s.arm(9310);
    assert!(hit(&mut s, 10.0, None, false, false));
    assert!(!hit(&mut s, 10.0, None, false, true), "damage over time");
    assert!(
        !hit(&mut s, 10.0, Some(SkillKey::new(9341, 1)), false, false),
        "allowSkillAttack is false"
    );
    assert!(
        !hit(&mut s, 10.0, None, true, false),
        "isCritical (false) must match the hit"
    );
    s.clear_triggered();
    s.world
        .notify_damage_received_by(ME, 10.0, Some(ME), None, false, false);
    assert!(!s.has(ME, TRIGGERED), "attacker == target");
    s.world.stop_skill_effects(ME, SkillFinishType::Silent, 9310);

    // `isCritical = true`: only critical hits.
    s.arm(9311);
    assert!(!hit(&mut s, 10.0, None, false, false));
    assert!(hit(&mut s, 10.0, None, true, false));
    s.world.stop_skill_effects(ME, SkillFinishType::Silent, 9311);

    // `minDamage = 50`.
    s.arm(9312);
    assert!(!hit(&mut s, 49.0, None, false, false));
    assert!(hit(&mut s, 50.0, None, false, false));
    s.world.stop_skill_effects(ME, SkillFinishType::Silent, 9312);

    // `attackerType = Attackable`: a player is no attackable.
    s.arm(9313);
    assert!(!hit(&mut s, 10.0, None, false, false));
    s.world.stop_skill_effects(ME, SkillFinishType::Silent, 9313);

    // `allowWeapons`: the player wields a sword.
    s.arm(9314);
    assert!(!hit(&mut s, 10.0, None, false, false), "BOW only");
    s.world.stop_skill_effects(ME, SkillFinishType::Silent, 9314);
    s.arm(9315);
    assert!(hit(&mut s, 10.0, None, false, false), "SWORD,BLUNT");
    s.world.stop_skill_effects(ME, SkillFinishType::Silent, 9315);

    // `chance = 0` never triggers.
    s.arm(9316);
    assert!(!hit(&mut s, 10.0, None, false, false));
}

/// The wire-in in `Creature.onHitTimer`: a landed hit tells the listeners who dealt it.
#[test]
fn a_landed_hit_feeds_the_damage_listeners() {
    let mut s = Scene::new();
    s.arm(9310);
    let hare = s.hare(0);
    s.world.on_hit_timer(HitTask {
        attacker: ME,
        target: hare,
        damage: 12,
        crit: false,
        miss: false,
        shld: 0,
        soulshot: false,
        recharge_shots: false,
    });
    assert!(s.has(ME, TRIGGERED));
}

// ---- TriggerSkillByDamageReceived ------------------------------------------------------------------

#[test]
fn damage_received_triggers_the_skill_on_the_victim() {
    let mut s = Scene::new();
    s.arm(9320);
    let hare = s.hare(0);
    s.world
        .notify_damage_received_by(ME, 10.0, Some(hare), None, false, false);
    assert!(s.has(ME, TRIGGERED));
    // Damage over time, damage from nobody and damage from oneself do not trigger.
    s.clear_triggered();
    s.world
        .notify_damage_received_by(ME, 10.0, Some(hare), None, false, true);
    s.world
        .notify_damage_received_by(ME, 10.0, None, None, false, false);
    s.world.notify_damage_received(ME, 10.0);
    s.world
        .notify_damage_received_by(ME, 10.0, Some(ME), None, false, false);
    assert!(!s.has(ME, TRIGGERED));
}

/// `minAttackerLevel = 50` and a level 1 attacker.
#[test]
fn damage_received_checks_the_attacker_level() {
    let mut s = Scene::new();
    s.arm(9321);
    let hare = s.hare(0);
    s.world
        .notify_damage_received_by(ME, 10.0, Some(hare), None, false, false);
    assert!(!s.has(ME, TRIGGERED));
}

/// The wire-in in `Creature.onHitTimer`: a hit on the owner reaches its listener.
#[test]
fn a_landed_hit_feeds_the_damage_received_listeners() {
    let mut s = Scene::new();
    s.arm(9320);
    let hare = s.hare(0);
    s.world.on_hit_timer(HitTask {
        attacker: hare,
        target: ME,
        damage: 12,
        crit: false,
        miss: false,
        shld: 0,
        soulshot: false,
        recharge_shots: false,
    });
    assert!(s.has(ME, TRIGGERED));
}

// ---- TriggerSkillByAvoid ---------------------------------------------------------------------------

#[test]
fn avoiding_an_attack_triggers_the_skill() {
    let mut s = Scene::new();
    s.arm(9330);
    let hare = s.hare(0);
    // A damage over time that missed does not count.
    s.world.notify_attack_avoid(hare, ME, true);
    assert!(!s.has(ME, TRIGGERED));
    s.world.notify_attack_avoid(hare, ME, false);
    assert!(s.has(ME, TRIGGERED));
}

/// The wire-in in `Creature.onHitTimer`: a missed hit is `notifyAttackAvoid`.
#[test]
fn a_missed_hit_feeds_the_avoid_listeners() {
    let mut s = Scene::new();
    s.arm(9330);
    let hare = s.hare(0);
    s.world.on_hit_timer(HitTask {
        attacker: hare,
        target: ME,
        damage: 0,
        crit: false,
        miss: true,
        shld: 0,
        soulshot: false,
        recharge_shots: false,
    });
    assert!(s.has(ME, TRIGGERED));
}

// ---- TriggerSkillBySkill ---------------------------------------------------------------------------

/// `Creature.beginCast` fires `ON_CREATURE_SKILL_USE`; the listener of the effect only answers to
/// its `castSkillId`.
#[test]
fn using_the_named_skill_triggers_the_skill() {
    let mut s = Scene::new();
    s.arm(9340);
    // Another skill does not trigger it.
    s.world.notify_skill_use(ME, SkillKey::new(9301, 1), OTHER);
    assert!(!s.has(ME, TRIGGERED));
    // The real cast does (`do_cast_at` is the triggered-cast entry of the casting code).
    s.world.do_cast_at(ME, SkillKey::new(9341, 1), OTHER, vec![OTHER]);
    assert!(s.has(ME, TRIGGERED));
}

// ---- TargetMe and the locked target ----------------------------------------------------------------

/// `TargetMe.onStart`: the playable targets the caster and cannot change or drop the target until
/// the effect ends (`Npc.canTarget`, `RequestTargetCanceld`).
#[test]
fn target_me_locks_the_target_of_a_player() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.apply(OTHER, ME, 9350);
    assert_eq!(s.world.target_of(ME), Some(OTHER));
    assert_eq!(s.world.locked_target(ME), Some(OTHER));
    assert!(s.world.is_locked_target(ME));

    // A click on an NPC fails with its message; so does dropping the target.
    s.log.take();
    s.input(
        ME,
        ClientInput::Action {
            object_id: hare.0,
            action_id: 0,
        },
    );
    let sent = s.run(100);
    assert!(message_ids(&sent, ME.0).contains(&SystemMessageId::FailedToChangeAttackTarget));
    assert_eq!(s.world.target_of(ME), Some(OTHER));
    s.input(ME, ClientInput::TargetCanceled { unselect: 1 });
    let sent = s.run(100);
    assert!(message_ids(&sent, ME.0).contains(&SystemMessageId::FailedToDisableAttackTarget));
    assert_eq!(s.world.target_of(ME), Some(OTHER));

    // The effect ends: free again.
    s.world.stop_skill_effects(ME, SkillFinishType::Removed, 9350);
    assert!(!s.world.is_locked_target(ME));
    s.input(
        ME,
        ClientInput::Action {
            object_id: hare.0,
            action_id: 0,
        },
    );
    s.run(100);
    assert_eq!(s.world.target_of(ME), Some(hare));
}

/// NOTE(java-quirk): an NPC caster has no `asPlayer()`, so Java's `setTarget(player)` clears the
/// playable's target instead of setting it.
#[test]
fn target_me_of_an_npc_clears_the_target() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.world.set_target(ME, Some(OTHER));
    assert_eq!(s.world.target_of(ME), Some(OTHER));
    s.apply(hare, ME, 9350);
    assert_eq!(s.world.target_of(ME), None);
    assert_eq!(s.world.locked_target(ME), Some(hare));
}

/// `TargetMeProbability` only switches the target, with no lock.
#[test]
fn target_me_probability_does_not_lock() {
    let mut s = Scene::new();
    s.apply(OTHER, ME, 9351);
    assert_eq!(s.world.target_of(ME), Some(OTHER));
    assert!(!s.world.is_locked_target(ME));
}

// ---- Resurrection and ResurrectionSpecial ----------------------------------------------------------

fn kill_me(s: &mut Scene) {
    let hare = s.hare(100);
    assert!(s.world.do_die(ME, Some(hare)));
    assert!(s.world.is_dead(ME));
    s.run(100);
    s.log.take();
}

fn confirm_dialogs(sent: &Sent, to: i32) -> Vec<ConfirmDlg> {
    of(sent, to, "CONFIRM_DLG")
        .into_iter()
        .map(decode::<ConfirmDlg>)
        .collect()
}

const RESURRECT_QUESTION: i32 = 1510;

/// `Resurrection.onStart` with a player caster: `Player.reviveRequest` asks the dead player with a
/// `ConfirmDlg` (the caster's name and the experience that comes back), and the `DlgAnswer`
/// revives it (`reviveAnswer`).
#[test]
fn resurrection_asks_the_dead_player_and_the_answer_revives() {
    let mut s = Scene::new();
    kill_me(&mut s);
    s.world.player_mut(ME).unwrap().data.profile.exp_before_death = 1000;
    s.apply(OTHER, ME, 9360);
    let sent = s.run(100);
    let dialogs = confirm_dialogs(&sent, ME.0);
    assert_eq!(dialogs.len(), 1);
    assert_eq!(dialogs[0].message.id.id(), RESURRECT_QUESTION);
    // 50% of the 1000 exp lost (the Wit bonus is 1 in this world).
    assert_eq!(
        dialogs[0].message.params,
        vec![
            SystemMessageParam::PlayerName("Other".into()),
            SystemMessageParam::Text("500".into())
        ]
    );
    assert!(s.world.is_revive_requested(ME));

    // A second request while one is open: the reviver is told, no second dialog.
    s.log.take();
    s.apply(OTHER, ME, 9360);
    let sent = s.run(100);
    assert!(message_ids(&sent, OTHER.0).contains(&SystemMessageId::ResurrectionHasAlreadyBeenProposed));
    assert_eq!(count(&sent, ME.0, "CONFIRM_DLG"), 0);

    // The answer comes back as `DlgAnswer` with the message id of the question.
    s.log.take();
    s.input(
        ME,
        ClientInput::DlgAnswer {
            message_id: RESURRECT_QUESTION,
            answer: 1,
            requester_id: 0,
        },
    );
    let sent = s.run(100);
    assert!(!s.world.is_dead(ME));
    assert!(!s.world.is_revive_requested(ME));
    assert_eq!(count(&sent, ME.0, "REVIVE"), 1);
}

#[test]
fn refusing_the_resurrection_keeps_the_player_dead() {
    let mut s = Scene::new();
    kill_me(&mut s);
    s.apply(OTHER, ME, 9360);
    s.input(
        ME,
        ClientInput::DlgAnswer {
            message_id: RESURRECT_QUESTION,
            answer: 0,
            requester_id: 0,
        },
    );
    s.run(100);
    assert!(s.world.is_dead(ME));
    assert!(!s.world.is_revive_requested(ME));
    // Another answer without a request does nothing.
    s.input(
        ME,
        ClientInput::DlgAnswer {
            message_id: RESURRECT_QUESTION,
            answer: 1,
            requester_id: 0,
        },
    );
    s.run(100);
    assert!(s.world.is_dead(ME));
}

/// A dialog for another question, or an answer from a living player, is ignored.
#[test]
fn only_the_resurrection_dialogs_are_answered() {
    let mut s = Scene::new();
    s.input(
        ME,
        ClientInput::DlgAnswer {
            message_id: RESURRECT_QUESTION,
            answer: 1,
            requester_id: 0,
        },
    );
    s.input(
        ME,
        ClientInput::DlgAnswer {
            message_id: 1,
            answer: 1,
            requester_id: 0,
        },
    );
    s.run(100);
    assert!(!s.world.is_dead(ME));
}

/// `BlockResurrection`: `isResurrectionBlocked()` makes `reviveRequest` return without a dialog.
#[test]
fn a_blocked_resurrection_asks_nothing() {
    let mut s = Scene::new();
    // The block stays after death (`stayAfterDeath`); a dead creature takes no new effects.
    s.apply(OTHER, ME, 9362);
    kill_me(&mut s);
    assert!(s.world.is_affected(ME, effect_flag::BLOCK_RESURRECTION));
    s.apply(OTHER, ME, 9360);
    assert_eq!(count(&s.run(100), ME.0, "CONFIRM_DLG"), 0);
    assert!(!s.world.is_revive_requested(ME));
}

/// `Resurrection.onStart` with an NPC caster: no dialog, the creature comes back at once
/// (`doRevive(percent)`).
#[test]
fn an_npc_resurrects_without_asking() {
    let mut s = Scene::new();
    kill_me(&mut s);
    let hare = s.hare(300);
    s.apply(hare, ME, 9360);
    assert_eq!(count(&s.run(100), ME.0, "CONFIRM_DLG"), 0);
    assert!(!s.world.is_dead(ME));
}

/// `ResurrectionSpecial.onExit`: when the buff ends (death ends it) the owner is asked to come
/// back; the effect also carries the `RESURRECTION_SPECIAL` flag.
#[test]
fn resurrection_special_asks_when_it_ends_at_death() {
    let mut s = Scene::new();
    s.apply(OTHER, ME, 9361);
    assert!(s.world.is_affected(ME, effect_flag::RESURRECTION_SPECIAL));
    let hare = s.hare(100);
    s.log.take();
    assert!(s.world.do_die(ME, Some(hare)));
    let sent = s.run(100);
    assert!(!s.world.is_affected(ME, effect_flag::RESURRECTION_SPECIAL));
    let dialogs = confirm_dialogs(&sent, ME.0);
    assert_eq!(dialogs.len(), 1);
    assert_eq!(dialogs[0].message.id.id(), RESURRECT_QUESTION);
    assert!(s.world.is_revive_requested(ME));
}

// ---- NevitsHourglass -------------------------------------------------------------------------------

/// `NevitsHourglass`: the hourglass of a player stands still while the effect lasts
/// (`setPausedNevitHourglassStatus`, which also sends `ExVoteSystemInfo`), and runs again after.
#[test]
fn nevits_hourglass_pauses_the_hourglass() {
    let mut s = Scene::new();
    s.log.take();
    s.apply(OTHER, ME, 9370);
    let sent = s.run(100);
    assert_eq!(s.world.nevit_hourglass_status(ME), 1);
    assert!(count(&sent, ME.0, "EX_VOTE_SYSTEM_INFO") >= 1);
    s.world.stop_skill_effects(ME, SkillFinishType::Removed, 9370);
    assert_ne!(s.world.nevit_hourglass_status(ME), 1);
    // Only players get it (`canStart`): the NPC's info stays without the effect.
    let hare = s.hare(100);
    s.apply(ME, hare, 9370);
    assert!(
        s.world
            .effect_list(hare)
            .is_none_or(|l| l.first_effect(EffectType::NevitsHourglass).is_none())
    );
    s.apply(OTHER, ME, 9370);
    assert!(
        s.world
            .effect_list(ME)
            .unwrap()
            .first_effect(EffectType::NevitsHourglass)
            .is_some()
    );
}

// ---- OpenChest -------------------------------------------------------------------------------------

/// `OpenChest`: a chest within six levels breaks open, drops its special loot and gives no exp.
#[test]
fn open_chest_breaks_a_chest_of_a_near_level() {
    let mut s = Scene::new();
    let chest = s.npc(9003, 100);
    assert!(s.world.npc(chest).unwrap().must_reward_exp_sp);
    s.log.take();
    s.apply(ME, chest, 9371);
    let sent = s.run(100);
    assert!(s.world.is_dead(chest));
    let npc = s.world.npc(chest).unwrap();
    assert!(npc.special_drop);
    assert!(!npc.must_reward_exp_sp);
    let social: SocialAction = decode(of(&sent, ME.0, "SOCIAL_ACTION")[0]);
    assert_eq!((social.object_id, social.action_id), (ME.0, 3));
}

/// A chest too strong for the player turns on it instead.
#[test]
fn open_chest_of_too_high_a_level_fights_back() {
    let mut s = Scene::new();
    let chest = s.npc(9004, 100);
    s.log.take();
    s.apply(ME, chest, 9371);
    let sent = s.run(100);
    assert!(!s.world.is_dead(chest));
    assert!(!s.world.npc(chest).unwrap().special_drop);
    let social: SocialAction = decode(of(&sent, ME.0, "SOCIAL_ACTION")[0]);
    assert_eq!(social.action_id, 13);
    assert_eq!(s.world.aggro_most_hated(chest), Some(ME));
}

/// Only chests can be opened.
#[test]
fn open_chest_ignores_other_creatures() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    s.log.take();
    s.apply(ME, hare, 9371);
    assert_eq!(count(&s.run(100), ME.0, "SOCIAL_ACTION"), 0);
    assert!(!s.world.is_dead(hare));
}

// ---- ConvertItem -----------------------------------------------------------------------------------

/// `ConvertItem`: the weapon in the hand becomes its `change_weaponId` twin with the same enchant
/// level and element, and the player is told.
#[test]
fn convert_item_swaps_the_weapon_and_keeps_enchant_and_element() {
    let mut s = Scene::new();
    s.world.set_item_elemental(OLD_SWORD, 2, 30);
    s.log.take();
    s.apply(OTHER, ME, 9372);
    let sent = s.run(200);

    let hand = s.world.paperdoll_object(ME, paperdoll::RHAND).unwrap();
    assert_ne!(hand, OLD_SWORD);
    let player = s.world.player(ME).unwrap();
    let new = player.data.items.iter().find(|i| i.object_id == hand).unwrap();
    assert_eq!((new.item_id, new.enchant_level), (3, 3));
    assert!(player.data.items.iter().all(|i| i.object_id != OLD_SWORD));
    assert_eq!(
        s.world.item_elemental(hand),
        Some(l2_world::effects::item_extras::ItemElemental {
            element: 2,
            value: 30
        })
    );
    assert_eq!(s.world.item_elemental(OLD_SWORD), None);

    // "The equipment +3 Sword has been removed.", then "Equipped +3 Changed Sword."
    let ids = message_ids(&sent, ME.0);
    let removed = ids
        .iter()
        .position(|i| *i == SystemMessageId::TheEquipmentS1S2HasBeenRemoved)
        .unwrap();
    let equipped = ids
        .iter()
        .position(|i| *i == SystemMessageId::EquippedS1S2)
        .unwrap();
    assert!(removed < equipped);
    assert!(count(&sent, ME.0, "INVENTORY_UPDATE") >= 1);
    assert!(
        count(&sent, OTHER.0, "CHAR_INFO") >= 1,
        "UserInfo goes to the others too"
    );
}

/// An augmented weapon, a weapon without a twin and a player without a weapon stay as they are.
#[test]
fn convert_item_leaves_the_other_weapons_alone() {
    let mut s = Scene::new();
    s.world.set_item_augmented(OLD_SWORD, true);
    s.apply(OTHER, ME, 9372);
    s.run(200);
    assert_eq!(s.world.paperdoll_object(ME, paperdoll::RHAND), Some(OLD_SWORD));

    // No weapon: nothing happens (the fists have no twin).
    s.apply(OTHER, OTHER, 9372);
    s.run(200);
    assert_eq!(s.world.paperdoll_object(OTHER, paperdoll::RHAND), None);
}

// ---- Grow ------------------------------------------------------------------------------------------

/// `Grow`: the NPC gets its grown collision radius while the effect lasts (`setCollisionRadius`),
/// the template's again after.
#[test]
fn grow_changes_the_collision_radius_of_an_npc() {
    let mut s = Scene::new();
    let hare = s.hare(100);
    assert_eq!(s.world.npc(hare).unwrap().collision_radius, 10.0);
    s.apply(ME, hare, 9373);
    assert_eq!(s.world.npc(hare).unwrap().collision_radius, 25.0);
    s.world.stop_skill_effects(hare, SkillFinishType::Removed, 9373);
    assert_eq!(s.world.npc(hare).unwrap().collision_radius, 10.0);
    // Death also resets it (`Npc.doDie`).
    s.apply(ME, hare, 9373);
    assert_eq!(s.world.npc(hare).unwrap().collision_radius, 25.0);
    assert!(s.world.do_die(hare, Some(ME)));
    assert_eq!(s.world.npc(hare).unwrap().collision_radius, 10.0);
}

// ---- RestorationRandom -----------------------------------------------------------------------------

fn item_count(s: &Scene, id: ObjectId, item_id: i32) -> i64 {
    s.world
        .player(id)
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == item_id)
        .map(|i| i.count)
        .sum()
}

/// `RestorationRandom`: the group the dice pick is given, the amounts times `RateExtractable`
/// (`(long) (count * rate)`), each item with "You have obtained".
#[test]
fn restoration_random_gives_the_picked_group_times_the_rate() {
    let mut s = Scene::with(EffectSettings {
        rate_extractable: 2.0,
        ..EffectSettings::default()
    });
    s.apply(OTHER, ME, 9374);
    let sent = s.run(200);
    assert_eq!(item_count(&s, ME, 1344), 20);
    assert_eq!(item_count(&s, ME, 1345), 6);
    let obtained = message_ids(&sent, ME.0)
        .into_iter()
        .filter(|i| *i == SystemMessageId::YouHaveObtainedS2S1)
        .count();
    assert_eq!(obtained, 2);
    assert!(count(&sent, ME.0, "INVENTORY_UPDATE") >= 1);
}

/// With the default rate the amount is the XML's.
#[test]
fn restoration_random_with_the_default_rate() {
    let mut s = Scene::new();
    s.apply(OTHER, ME, 9374);
    s.run(200);
    assert_eq!((item_count(&s, ME, 1344), item_count(&s, ME, 1345)), (10, 3));
}

/// No group picked: "There was nothing found inside."
#[test]
fn restoration_random_can_find_nothing() {
    let mut s = Scene::new();
    s.apply(OTHER, ME, 9375);
    let sent = s.run(200);
    assert!(message_ids(&sent, ME.0).contains(&SystemMessageId::ThereWasNothingFoundInside));
    assert_eq!(item_count(&s, ME, 1344), 0);
}

// ---- FlyToLocation ---------------------------------------------------------------------------------

fn fly_packets(sent: &Sent, to: i32) -> Vec<FlyToLocation> {
    of(sent, to, "FLY_TO_LOCATION")
        .into_iter()
        .map(decode::<FlyToLocation>)
        .collect()
}

/// `Blink`: `FlyToLocation` (`DUMMY`) from where the player stands, then the new position and
/// `ValidateLocation`.
#[test]
fn blink_broadcasts_the_fly_before_the_validate_location() {
    let mut s = Scene::new();
    s.log.take();
    s.apply(ME, ME, 9380);
    let sent = s.run(100);
    let flies = fly_packets(&sent, OTHER.0);
    assert_eq!(flies.len(), 1);
    assert_eq!((flies[0].object_id, flies[0].fly_type), (ME.0, 2));
    assert_eq!((flies[0].org_x, flies[0].org_y, flies[0].org_z), (X, Y, Z));
    let fly_at = sent
        .iter()
        .position(|(to, b)| *to == OTHER.0 && name_of(b) == "FLY_TO_LOCATION");
    let validate_at = sent
        .iter()
        .position(|(to, b)| *to == OTHER.0 && name_of(b) == "VALIDATE_LOCATION");
    assert!(fly_at.unwrap() < validate_at.unwrap());
    // The player itself hears it too.
    assert_eq!(fly_packets(&sent, ME.0).len(), 1);
}

/// `ThrowUp` flies the victim as `THROW_UP`; `TeleportToTarget` flies the caster as `DUMMY`.
#[test]
fn throw_up_and_teleport_to_target_broadcast_the_fly() {
    let mut s = Scene::new();
    s.log.take();
    s.apply(ME, OTHER, 9381);
    let sent = s.run(100);
    let flies = fly_packets(&sent, ME.0);
    assert_eq!(flies.len(), 1);
    assert_eq!((flies[0].object_id, flies[0].fly_type), (OTHER.0, 0));

    s.log.take();
    s.apply(ME, OTHER, 9382);
    let sent = s.run(100);
    let flies = fly_packets(&sent, OTHER.0);
    assert_eq!(flies.len(), 1);
    assert_eq!((flies[0].object_id, flies[0].fly_type), (ME.0, 2));
}

// ---- CancelReturn ----------------------------------------------------------------------------------

fn cancel_return(on: bool) -> EffectSettings {
    EffectSettings {
        cancel_return_on: on,
        cancel_return_time_ms: 1000,
        ..EffectSettings::default()
    }
}

/// Without `CancelReturn` a dispel is final.
#[test]
fn a_dispel_is_final_when_cancel_return_is_off() {
    let mut s = Scene::with(cancel_return(false));
    s.apply(OTHER, ME, TRIGGERED);
    assert!(s.has(ME, TRIGGERED));
    s.apply(OTHER, ME, 9390);
    assert!(!s.has(ME, TRIGGERED));
    s.run(3000);
    assert!(!s.has(ME, TRIGGERED));
}

/// `DispelAll` with `CancelReturn`: the buff comes back after `TimeToReturn` with the time it had
/// left (`BuffInfo.getTime()`).
#[test]
fn dispel_all_gives_the_buffs_back() {
    let mut s = Scene::with(cancel_return(true));
    s.apply(OTHER, ME, TRIGGERED);
    s.run(5000);
    s.apply(OTHER, ME, 9390);
    assert!(!s.has(ME, TRIGGERED));
    s.run(500);
    assert!(!s.has(ME, TRIGGERED), "too early");
    s.run(1000);
    assert!(s.has(ME, TRIGGERED));
    // 60 s, 5 s before the dispel and 1 s until it returned: 54 s left.
    let left = s
        .world
        .effect_list(ME)
        .unwrap()
        .info_by_skill_id(TRIGGERED)
        .unwrap()
        .abnormal_time();
    assert_eq!(left, 54);
}

/// The same for `DispelBySlotProbability`, and not for a player that died meanwhile.
#[test]
fn dispel_by_slot_probability_gives_the_buff_back_unless_the_player_died() {
    let mut s = Scene::with(cancel_return(true));
    s.apply(OTHER, ME, TRIGGERED);
    s.apply(OTHER, ME, 9391);
    assert!(!s.has(ME, TRIGGERED));
    s.run(1100);
    assert!(s.has(ME, TRIGGERED));

    s.world.stop_skill_effects(ME, SkillFinishType::Silent, TRIGGERED);
    s.apply(OTHER, ME, TRIGGERED);
    s.apply(OTHER, ME, 9391);
    let hare = s.hare(100);
    assert!(s.world.do_die(ME, Some(hare)));
    s.run(1500);
    assert!(!s.has(ME, TRIGGERED));
}

/// Every synthetic skill of the file parses (a bad abnormal type drops the rest of the list).
#[test]
fn the_test_skills_all_load() {
    let skills = SkillTemplates::from_xml(
        &skills_xml(),
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    );
    assert_eq!(skills.len(), skills_xml().matches("<skill ").count());
}

/// `ReturnPlayer = False`: a player's dispel is final.
#[test]
fn cancel_return_follows_the_player_switch() {
    let mut s = Scene::with(EffectSettings {
        cancel_return_player: false,
        ..cancel_return(true)
    });
    s.apply(OTHER, ME, TRIGGERED);
    s.apply(OTHER, ME, 9390);
    s.run(2000);
    assert!(!s.has(ME, TRIGGERED));
}
